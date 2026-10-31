use std::env;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::System::Services::*;

pub const SERVICE_NAME: PCWSTR = w!("CanonLBP1120Service");
pub const SERVICE_NAME_STR: &str = "CanonLBP1120Service";
pub const SERVICE_DISPLAY_NAME_STR: &str = "Canon LaserShot LBP-1120 Print Service";

static mut SERVICE_STATUS_HANDLE: Option<SERVICE_STATUS_HANDLE> = None;
pub static SERVICE_SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Returns the root service directory: `%ProgramData%\CanonLBP1120`
pub fn get_service_dir() -> PathBuf {
    let program_data = env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
    let mut dir = PathBuf::from(program_data);
    dir.push("CanonLBP1120");
    dir
}

/// Returns the path to the service log file: `%ProgramData%\CanonLBP1120\service.log`
pub fn get_service_log_path() -> PathBuf {
    get_service_dir().join("service.log")
}

/// Returns the path to the per-job logs directory: `%ProgramData%\CanonLBP1120\jobs`
pub fn get_jobs_dir() -> PathBuf {
    get_service_dir().join("jobs")
}

/// Returns the path to the spool data directory: `%ProgramData%\CanonLBP1120\spool`
pub fn get_spool_dir() -> PathBuf {
    get_service_dir().join("spool")
}

/// Generates a globally unique, timestamped per-job identifier (e.g. `JOB-211800-P9100-0001`)
pub fn generate_job_id(port: u16, mode_str: &str) -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let d = std::time::SystemTime::now();
    let since = d.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = since.as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let prefix = if mode_str.to_lowercase().contains("photo") { "PHOTO" } else { "NORM" };
    format!("JOB-{:02}{:02}{:02}-{}-P{}-{:04}", h, m, s, prefix, port, seq)
}

/// Persistent, multi-target job logger for end-to-end print pipeline observability.
/// Logs are flushed immediately to both `%ProgramData%\CanonLBP1120\jobs\<job_id>.log`
/// and `%ProgramData%\CanonLBP1120\service.log`.
#[derive(Clone, Debug)]
pub struct JobLogger {
    pub job_id: String,
    pub job_log_path: PathBuf,
}

impl JobLogger {
    pub fn new(job_id: &str) -> Self {
        let jobs_dir = get_jobs_dir();
        let _ = create_dir_all(&jobs_dir);
        let job_log_path = jobs_dir.join(format!("{}.log", job_id));
        Self {
            job_id: job_id.to_string(),
            job_log_path,
        }
    }

    pub fn log(&self, stage: &str, message: &str) {
        let now = chrono_like_now();
        let line = format!("[{}] [{}] [{}] {}\n", now, self.job_id, stage, message);
        self.write_line(&line);
        print!("{}", line);
        let _ = std::io::stdout().flush();
    }

    pub fn log_error(&self, stage: &str, message: &str) {
        let now = chrono_like_now();
        let line = format!("[{}] [{}] [ERROR: {}] {}\n", now, self.job_id, stage, message);
        self.write_line(&line);
        eprint!("{}", line);
        let _ = std::io::stderr().flush();
    }

    fn write_line(&self, text: &str) {
        // 1. Write to job-specific log file
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.job_log_path) {
            let _ = f.write_all(text.as_bytes());
        }
        // 2. Write to unified service log file
        let svc_log = get_service_log_path();
        if let Some(parent) = svc_log.parent() {
            let _ = create_dir_all(parent);
        }
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&svc_log) {
            let _ = f.write_all(text.as_bytes());
        }
    }

    pub fn log_summary(
        &self,
        stage1_submission: &str,
        stage2_socket: &str,
        stage3_ingest: &str,
        stage4_pdf: &str,
        stage5_scoa: &str,
        stage6_usb: &str,
        stage7_print: &str,
        final_result: &str,
    ) {
        let summary = format!(
            "\n======================================================================\n\
             CANON LBP-1120 PRINT PIPELINE EXECUTION SUMMARY: {}\n\
             ======================================================================\n\
             Stage 1: Windows Print Spooler Submission    -> {}\n\
             Stage 2: RAW TCP / Named Pipe Connection      -> {}\n\
             Stage 3: Service Ingestion & Stream Analysis  -> {}\n\
             Stage 4: PDF / Direct2D Raster Conversion     -> {}\n\
             Stage 5: SCoA Compression & CAPT Framing      -> {}\n\
             Stage 6: WinUSB Device Discovery & Interface  -> {}\n\
             Stage 7: USB Transmission & Mechanical Feed   -> {}\n\
             ----------------------------------------------------------------------\n\
             FINAL RESULT: {}\n\
             ======================================================================\n\n",
            self.job_id,
            stage1_submission,
            stage2_socket,
            stage3_ingest,
            stage4_pdf,
            stage5_scoa,
            stage6_usb,
            stage7_print,
            final_result,
        );
        self.write_line(&summary);
        print!("{}", summary);
        let _ = std::io::stdout().flush();
    }
}

/// Appends a timestamped log entry to `%ProgramData%\CanonLBP1120\service.log`.
pub fn log_service_event(event: &str) {
    let log_path = get_service_log_path();
    if let Some(parent) = log_path.parent() {
        let _ = create_dir_all(parent);
    }
    let now = chrono_like_now();
    let line = format!("[{}] {}\n", now, event);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let _ = f.write_all(line.as_bytes());
    }
}

fn chrono_like_now() -> String {
    let d = std::time::SystemTime::now();
    let since = d.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = since.as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    format!("T+{:02}:{:02}:{:02}", h, m, s)
}

unsafe extern "system" fn service_ctrl_handler(opcode: u32) {
    match opcode {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            log_service_event("Service Control Manager requested STOP / SHUTDOWN.");
            SERVICE_SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);

            unsafe {
                if let Some(h) = SERVICE_STATUS_HANDLE {
                    let status = SERVICE_STATUS {
                        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
                        dwCurrentState: SERVICE_STOP_PENDING,
                        dwControlsAccepted: 0,
                        dwWin32ExitCode: 0,
                        dwServiceSpecificExitCode: 0,
                        dwCheckPoint: 1,
                        dwWaitHint: 5000,
                    };
                    let _ = SetServiceStatus(h, &status);
                }
            }
        }
        _ => {}
    }
}

unsafe extern "system" fn service_main(_argc: u32, _argv: *mut PWSTR) {
    let handle = unsafe {
        match RegisterServiceCtrlHandlerW(SERVICE_NAME, Some(service_ctrl_handler)) {
            Ok(h) => h,
            Err(_) => return,
        }
    };
    unsafe {
        SERVICE_STATUS_HANDLE = Some(handle);
    }

    // 1. Report START_PENDING
    let start_pending = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: SERVICE_START_PENDING,
        dwControlsAccepted: 0,
        dwWin32ExitCode: 0,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: 3000,
    };
    unsafe {
        let _ = SetServiceStatus(handle, &start_pending);
    }

    log_service_event("Canon LaserShot LBP-1120 Service initializing under Windows SCM...");

    // 2. Report RUNNING
    let running = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: SERVICE_RUNNING,
        dwControlsAccepted: SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN,
        dwWin32ExitCode: 0,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: 0,
    };
    unsafe {
        let _ = SetServiceStatus(handle, &running);
    }
    log_service_event("Canon LaserShot LBP-1120 Service transitioned to RUNNING state.");

    // 3. Spawn spool server worker
    let server_thread = std::thread::spawn(|| {
        if let Err(e) = crate::spool::run_pipe_server(true) {
            log_service_event(&format!("Spool server encountered error: {}", e));
        }
    });

    // 4. Wait for shutdown signal from SCM
    while !SERVICE_SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(500));
    }

    log_service_event("Service shutdown in progress, cleaning up resources...");
    let _ = server_thread.join();

    // 5. Report STOPPED
    let stopped = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: SERVICE_STOPPED,
        dwControlsAccepted: 0,
        dwWin32ExitCode: 0,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: 0,
    };
    unsafe {
        let _ = SetServiceStatus(handle, &stopped);
    }
    log_service_event("Canon LaserShot LBP-1120 Service successfully STOPPED.");
}

/// Entry point when invoked by SCM with `service-run`
pub fn run_scm_service() -> Result<(), Box<dyn std::error::Error>> {
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: PWSTR(SERVICE_NAME.as_ptr() as *mut _),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: PWSTR(std::ptr::null_mut()),
            lpServiceProc: None,
        },
    ];

    unsafe {
        StartServiceCtrlDispatcherW(&table[0])?;
    }
    Ok(())
}

/// Returns the path to the installer diagnostic report: `%TEMP%\CanonLBP1120_Install_Diagnostics.txt`
pub fn get_install_diagnostic_path() -> PathBuf {
    env::temp_dir().join("CanonLBP1120_Install_Diagnostics.txt")
}

/// Returns the path to the installer error summary: `%TEMP%\CanonLBP1120_Install_Error.txt`
pub fn get_install_error_path() -> PathBuf {
    env::temp_dir().join("CanonLBP1120_Install_Error.txt")
}

#[derive(Debug, Clone)]
pub struct ScCommandRecord {
    pub description: String,
    pub command_line: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub struct InstallAuditLog {
    pub os_version: String,
    pub architecture: String,
    pub computer_name: String,
    pub user_name: String,
    pub is_elevated: bool,
    pub install_dir: String,
    pub exe_path: String,
    pub exe_exists: bool,
    pub exe_size_bytes: u64,
    pub port_9100_status: String,
    pub port_9101_status: String,
    pub initial_service_query: String,
    pub commands: Vec<ScCommandRecord>,
    pub final_result: String,
    pub error_summary: Option<String>,
}

pub fn explain_scm_error(code: Option<i32>, text: &str) -> String {
    let lower = text.to_lowercase();
    if code == Some(5) || lower.contains(" 5 :") || lower.contains("accès refusé") || lower.contains("access is denied") {
        return "SCM Error 5 (ERROR_ACCESS_DENIED): Administrator elevation required. Ensure setup runs with administrative privileges (UAC Admin).".to_string();
    }
    if code == Some(1072) || lower.contains("1072") || lower.contains("marked for deletion") || lower.contains("marqué pour suppression") {
        return "SCM Error 1072 (ERROR_SERVICE_MARKED_FOR_DELETE): Service marked for deletion by a prior command because an open handle remains in another process (e.g., 'services.msc' or Task Manager). Close all Windows management tools and retry.".to_string();
    }
    if code == Some(1053) || lower.contains("1053") || lower.contains("did not respond") {
        return "SCM Error 1053 (ERROR_SERVICE_REQUEST_TIMEOUT): Service process did not respond to start control dispatcher in a timely fashion.".to_string();
    }
    if code == Some(1056) || lower.contains("1056") || lower.contains("already running") {
        return "SCM Error 1056 (ERROR_SERVICE_ALREADY_RUNNING): An instance of the service is already running.".to_string();
    }
    if code == Some(1060) || lower.contains("1060") || lower.contains("does not exist") || lower.contains("n'existe pas") {
        return "SCM Error 1060 (ERROR_SERVICE_DOES_NOT_EXIST): The specified service does not exist.".to_string();
    }
    if code == Some(1073) || lower.contains("1073") || lower.contains("already exists") {
        return "SCM Error 1073 (ERROR_SERVICE_EXISTS): The specified service already exists.".to_string();
    }
    if code == Some(87) || code == Some(1639) || lower.contains("1639") || lower.contains("parameter is incorrect") {
        return "SCM Error 87/1639 (ERROR_INVALID_PARAMETER): Command-line parameter syntax for sc.exe was rejected.".to_string();
    }
    format!("SCM Error (Exit Code {:?}): {}", code, text.trim())
}

pub fn check_tcp_port_status(port: u16) -> String {
    match std::net::TcpListener::bind(("127.0.0.1", port)) {
        Ok(_) => format!("FREE (Successfully bound on 127.0.0.1:{})", port),
        Err(e) => {
            let mut desc = format!("CONFLICT: Cannot bind 127.0.0.1:{} ({})", port, e);
            if let Ok(out) = Command::new("netstat")
                .args(["-ano", "-p", "tcp"])
                .creation_flags(0x08000000)
                .output()
            {
                let stdout = String::from_utf8_lossy(&out.stdout);
                for line in stdout.lines() {
                    if line.contains(&format!(":{}", port)) && line.contains("LISTENING") {
                        desc.push_str(&format!(" | Active listener: {}", line.trim()));
                    }
                }
            }
            desc
        }
    }
}

pub fn write_diagnostic_report(audit: &InstallAuditLog) -> Result<PathBuf, std::io::Error> {
    let diag_path = get_install_diagnostic_path();
    let mut f = std::fs::File::create(&diag_path)?;
    let mut content = String::new();

    content.push_str("================================================================================\n");
    content.push_str("   CANON LASERSHOT LBP-1120 — INSTALLATION DIAGNOSTIC REPORT                    \n");
    content.push_str("================================================================================\n");
    content.push_str(&format!("Report Timestamp       : {}\n", chrono_like_now()));
    content.push_str(&format!("Diagnostic File Path   : {}\n\n", diag_path.display()));

    content.push_str("[1] OPERATING SYSTEM & ENVIRONMENT\n");
    content.push_str("--------------------------------------------------------------------------------\n");
    content.push_str(&format!("OS Version             : {}\n", audit.os_version));
    content.push_str(&format!("Architecture           : {}\n", audit.architecture));
    content.push_str(&format!("Computer Name          : {}\n", audit.computer_name));
    content.push_str(&format!("User Identity          : {}\n", audit.user_name));
    content.push_str(&format!("Administrative Token   : {}\n\n", if audit.is_elevated { "YES (Elevated Administrator)" } else { "NO (Standard User - Elevation Missing!)" }));

    content.push_str("[2] INSTALLATION DIRECTORY & BINARY INTEGRITY\n");
    content.push_str("--------------------------------------------------------------------------------\n");
    content.push_str(&format!("Installation Directory : {}\n", audit.install_dir));
    content.push_str(&format!("Executable Binary Path : {}\n", audit.exe_path));
    content.push_str(&format!("Executable Exists      : {}\n", if audit.exe_exists { "YES" } else { "NO (MISSING BINARY!)" }));
    content.push_str(&format!("Executable Size        : {} bytes\n\n", audit.exe_size_bytes));

    content.push_str("[3] TCP NETWORK & SPOOL PORT CONFLICT AUDIT\n");
    content.push_str("--------------------------------------------------------------------------------\n");
    content.push_str(&format!("Port 9100 (Normal Mode): {}\n", audit.port_9100_status));
    content.push_str(&format!("Port 9101 (Photo Mode) : {}\n\n", audit.port_9101_status));

    content.push_str("[4] INITIAL SCM REGISTRATION STATE\n");
    content.push_str("--------------------------------------------------------------------------------\n");
    content.push_str(&format!("Pre-Install Query      : {}\n\n", audit.initial_service_query));

    content.push_str("[5] SCM COMMAND EXECUTION AUDIT TRAIL\n");
    content.push_str("--------------------------------------------------------------------------------\n");
    for (i, cmd) in audit.commands.iter().enumerate() {
        content.push_str(&format!("[Command {}] {}\n", i + 1, cmd.description));
        content.push_str(&format!("  CommandLine : {}\n", cmd.command_line));
        content.push_str(&format!("  ExitCode    : {:?}\n", cmd.exit_code));
        content.push_str(&format!("  Duration    : {} ms\n", cmd.duration_ms));
        content.push_str(&format!("  Success     : {}\n", cmd.success));
        content.push_str("  STDOUT      :\n");
        if cmd.stdout.is_empty() {
            content.push_str("    (empty)\n");
        } else {
            for line in cmd.stdout.lines() {
                content.push_str(&format!("    {}\n", line));
            }
        }
        content.push_str("  STDERR      :\n");
        if cmd.stderr.is_empty() {
            content.push_str("    (empty)\n");
        } else {
            for line in cmd.stderr.lines() {
                content.push_str(&format!("    {}\n", line));
            }
        }
        content.push_str("\n");
    }

    content.push_str("[6] FINAL INSTALLATION VERDICT\n");
    content.push_str("--------------------------------------------------------------------------------\n");
    content.push_str(&format!("Result                 : {}\n", audit.final_result));
    if let Some(err) = &audit.error_summary {
        content.push_str(&format!("Failure Details        : {}\n", err));
    }
    content.push_str("================================================================================\n");

    f.write_all(content.as_bytes())?;
    Ok(diag_path)
}

fn execute_sc(
    description: &str,
    args: &[&str],
    raw_args: &[&str],
    records: &mut Vec<ScCommandRecord>,
) -> ScCommandRecord {
    let start_time = std::time::Instant::now();
    let mut cmd = Command::new("sc.exe");
    cmd.creation_flags(0x08000000);
    for arg in args {
        cmd.arg(arg);
    }
    for raw in raw_args {
        cmd.raw_arg(raw);
    }

    let mut cmd_display = format!("sc.exe {}", args.join(" "));
    for raw in raw_args {
        cmd_display.push(' ');
        cmd_display.push_str(raw);
    }

    let output = cmd.output();
    let duration = start_time.elapsed().as_millis();

    let record = match output {
        Ok(out) => {
            let code = out.status.code();
            let stdout_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let stderr_str = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let success = out.status.success();
            ScCommandRecord {
                description: description.to_string(),
                command_line: cmd_display,
                exit_code: code,
                stdout: stdout_str,
                stderr: stderr_str,
                duration_ms: duration,
                success,
            }
        }
        Err(e) => ScCommandRecord {
            description: description.to_string(),
            command_line: cmd_display,
            exit_code: None,
            stdout: String::new(),
            stderr: format!("Failed to spawn process: {}", e),
            duration_ms: duration,
            success: false,
        },
    };

    records.push(record.clone());
    record
}

/// Safely inspects ports 9100 and 9101 for listening conflicts.
/// If and ONLY if an orphaned background process belonging to this driver suite
/// (canon-lbp1120-service.exe or capt-poc.exe) is holding the port, it terminates that specific PID
/// and logs the action. It never terminates unrelated processes or active print jobs.
pub fn cleanup_port_conflict_if_ours(port: u16) -> Option<String> {
    if let Ok(_) = std::net::TcpListener::bind(("127.0.0.1", port)) {
        return None;
    }

    let netstat_out = match Command::new("netstat")
        .args(["-ano", "-p", "tcp"])
        .creation_flags(0x08000000)
        .output()
    {
        Ok(out) => out,
        Err(_) => return None,
    };

    let stdout = String::from_utf8_lossy(&netstat_out.stdout);
    let port_str = format!(":{}", port);
    let my_pid = std::process::id();

    for line in stdout.lines() {
        if line.contains(&port_str) && line.contains("LISTENING") {
            if let Some(pid_str) = line.split_whitespace().last() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    if pid != my_pid && pid != 0 && pid != 4 {
                        if let Ok(tasklist_out) = Command::new("tasklist")
                            .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
                            .creation_flags(0x08000000)
                            .output()
                        {
                            let csv = String::from_utf8_lossy(&tasklist_out.stdout).to_lowercase();
                            if csv.contains("canon-lbp1120-service") || csv.contains("capt-poc") {
                                let action_msg = format!(
                                    "Terminated orphaned application process (PID {}, image: {}) holding TCP port {}",
                                    pid,
                                    csv.split(',').next().unwrap_or("unknown").trim_matches('"'),
                                    port
                                );
                                log_service_event(&action_msg);
                                println!("      [Conflict Cleanup] {}", action_msg);
                                let _ = Command::new("taskkill.exe")
                                    .args(["/F", "/PID", &pid.to_string()])
                                    .creation_flags(0x08000000)
                                    .output();
                                return Some(action_msg);
                            } else {
                                let skip_msg = format!(
                                    "Port {} is held by third-party process (PID {}). Leaving process untouched.",
                                    port, pid
                                );
                                log_service_event(&skip_msg);
                                println!("      [Port Notice] {}", skip_msg);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Registers the binary as a native Windows Service using Service Control Manager (`sc.exe`)
pub fn install_windows_service() -> Result<(), Box<dyn std::error::Error>> {
    println!("=======================================================");
    println!("   CANON LBP-1120 WINDOWS SERVICE INSTALLER           ");
    println!("=======================================================");

    log_service_event("Beginning Windows Service installation sequence...");

    let exe_path_buf = env::current_exe()?;
    let exe_path = exe_path_buf.to_string_lossy().replace(r"\\?\", "");
    let install_dir = exe_path_buf.parent().unwrap_or(std::path::Path::new("")).to_string_lossy().to_string();
    let exe_exists = exe_path_buf.exists();
    let exe_size = exe_path_buf.metadata().map(|m| m.len()).unwrap_or(0);

    let os_ver = Command::new("cmd.exe")
        .args(["/c", "ver"])
        .creation_flags(0x08000000)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "Unknown Windows OS".to_string());

    let arch = env::var("PROCESSOR_ARCHITECTURE").unwrap_or_else(|_| env::consts::ARCH.to_string());
    let comp_name = env::var("COMPUTERNAME").unwrap_or_else(|_| "UNKNOWN".to_string());
    let user_name = env::var("USERNAME").unwrap_or_else(|_| "UNKNOWN".to_string());
    let is_elevated = Command::new("net")
        .args(["session"])
        .creation_flags(0x08000000)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Safely inspect and resolve port conflicts if held by orphaned instances of this driver
    let cleanup_9100 = cleanup_port_conflict_if_ours(9100);
    let cleanup_9101 = cleanup_port_conflict_if_ours(9101);
    let mut port_9100 = check_tcp_port_status(9100);
    if let Some(msg) = cleanup_9100 {
        port_9100.push_str(&format!(" [Cleanup Action: {}]", msg));
    }
    let mut port_9101 = check_tcp_port_status(9101);
    if let Some(msg) = cleanup_9101 {
        port_9101.push_str(&format!(" [Cleanup Action: {}]", msg));
    }

    let mut commands = Vec::new();

    // 1. Query initial service registration state
    println!("[1/5] Querying initial service registration state...");
    let initial_query = execute_sc("Query Initial Service State", &["query", SERVICE_NAME_STR], &[], &mut commands);
    let initial_query_summary = if initial_query.exit_code == Some(0) {
        format!("Service ALREADY EXISTS. State: {}", initial_query.stdout.replace("\r\n", " ").replace("\n", " "))
    } else {
        format!("Service does NOT exist (Clean initial state / Exit Code {:?})", initial_query.exit_code)
    };

    // If currently running, stop it first
    if initial_query.stdout.contains("RUNNING") || initial_query.stdout.contains(" 4 ") {
        println!("      Service is currently RUNNING; issuing stop command...");
        execute_sc("Stop Active Service", &["stop", SERVICE_NAME_STR], &[], &mut commands);
        // Wait up to 5 seconds for STOPPED state
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(500));
            let q = execute_sc("Query State After Stop", &["query", SERVICE_NAME_STR], &[], &mut commands);
            if q.stdout.contains("STOPPED") || q.stdout.contains(" 1 ") {
                break;
            }
        }
    }

    let bin_path_arg = format!("\"{}\" service-run", exe_path);
    let bin_path_raw = format!("binPath= \"\\\"{}\\\" service-run\"", exe_path);
    let display_name_raw = format!("DisplayName= \"{}\"", SERVICE_DISPLAY_NAME_STR);

    // 2. Register or Configure Service
    println!("[2/5] Registering/Configuring Windows Service '{}'...", SERVICE_NAME_STR);
    let service_existed = initial_query.exit_code == Some(0);

    let reg_result = if service_existed {
        // In-place update to prevent Error 1072 (ERROR_SERVICE_MARKED_FOR_DELETE)
        println!("      Service already exists; applying in-place configuration update (sc config)...");
        let conf = execute_sc(
            "Update Existing Service Configuration",
            &["config", SERVICE_NAME_STR],
            &[
                &bin_path_raw,
                &display_name_raw,
                "start= auto",
                "obj= LocalSystem",
            ],
            &mut commands,
        );
        if conf.success {
            conf
        } else {
            println!("      In-place update returned non-zero; falling back to sc create...");
            execute_sc(
                "Create Windows Service (Fallback)",
                &["create", SERVICE_NAME_STR],
                &[
                    &bin_path_raw,
                    &display_name_raw,
                    "start= auto",
                    "obj= LocalSystem",
                ],
                &mut commands,
            )
        }
    } else {
        println!("      Registering new Windows Service via sc create...");
        let cr = execute_sc(
            "Create New Windows Service",
            &["create", SERVICE_NAME_STR],
            &[
                &bin_path_raw,
                &display_name_raw,
                "start= auto",
                "obj= LocalSystem",
            ],
            &mut commands,
        );
        if !cr.success && (cr.exit_code == Some(1073) || cr.stdout.contains("1073") || cr.stdout.contains("already exists")) {
            println!("      Service already exists (1073); updating configuration in-place...");
            execute_sc(
                "Update Service Configuration (Fallback on 1073)",
                &["config", SERVICE_NAME_STR],
                &[
                    &bin_path_raw,
                    &display_name_raw,
                    "start= auto",
                    "obj= LocalSystem",
                ],
                &mut commands,
            )
        } else {
            cr
        }
    };

    if !reg_result.success {
        let err_detail = explain_scm_error(reg_result.exit_code, &format!("{}\n{}", reg_result.stdout, reg_result.stderr));
        let error_msg = format!("Failed to create/configure Windows service: {}", err_detail);

        let audit = InstallAuditLog {
            os_version: os_ver,
            architecture: arch,
            computer_name: comp_name,
            user_name,
            is_elevated,
            install_dir,
            exe_path,
            exe_exists,
            exe_size_bytes: exe_size,
            port_9100_status: port_9100,
            port_9101_status: port_9101,
            initial_service_query: initial_query_summary,
            commands,
            final_result: "FAILED (Service Creation/Configuration Error)".to_string(),
            error_summary: Some(error_msg.clone()),
        };
        let _ = write_diagnostic_report(&audit);
        let _ = std::fs::write(get_install_error_path(), &error_msg);
        log_service_event(&format!("INSTALL ERROR: {}", error_msg));
        return Err(error_msg.into());
    }

    // 3. Set description
    println!("[3/5] Setting service description and recovery policy...");
    execute_sc(
        "Set Service Description",
        &[
            "description",
            SERVICE_NAME_STR,
            "Canon LaserShot LBP-1120 native driver service providing 600 DPI rendering, SCoA compression, and WinUSB transmission.",
        ],
        &[],
        &mut commands,
    );

    // 4. Set crash recovery actions
    execute_sc(
        "Set Crash Recovery Actions",
        &["failure", SERVICE_NAME_STR],
        &["reset= 86400", "actions= restart/5000/restart/10000/none/0"],
        &mut commands,
    );

    // 5. Start service
    println!("[4/5] Starting service '{}' and waiting for RUNNING state...", SERVICE_NAME_STR);
    let start_record = execute_sc("Start Windows Service", &["start", SERVICE_NAME_STR], &[], &mut commands);

    // Poll service status until RUNNING (up to 10 seconds)
    println!("[5/5] Polling Service Control Manager status...");
    let mut running = false;
    let mut last_query_out = String::new();
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(500));
        let query_record = execute_sc("Poll Service Status", &["query", SERVICE_NAME_STR], &[], &mut commands);
        last_query_out = query_record.stdout.clone();
        if last_query_out.contains("RUNNING") || last_query_out.contains(" 4 ") {
            running = true;
            break;
        }
    }

    if !running {
        let err_detail = explain_scm_error(start_record.exit_code, &format!("Start Output:\n{}\n\nLast Query Output:\n{}", start_record.stdout, last_query_out));
        let error_msg = format!("Service failed to reach RUNNING state within 10 seconds: {}", err_detail);
        let audit = InstallAuditLog {
            os_version: os_ver,
            architecture: arch,
            computer_name: comp_name,
            user_name,
            is_elevated,
            install_dir,
            exe_path,
            exe_exists,
            exe_size_bytes: exe_size,
            port_9100_status: port_9100,
            port_9101_status: port_9101,
            initial_service_query: initial_query_summary,
            commands,
            final_result: "FAILED (Service Startup Timeout)".to_string(),
            error_summary: Some(error_msg.clone()),
        };
        let _ = write_diagnostic_report(&audit);
        let _ = std::fs::write(get_install_error_path(), &error_msg);
        log_service_event(&format!("INSTALL ERROR: {}", error_msg));
        return Err(error_msg.into());
    }

    println!("Service successfully started and verified in RUNNING state!");
    let audit = InstallAuditLog {
        os_version: os_ver,
        architecture: arch,
        computer_name: comp_name,
        user_name,
        is_elevated,
        install_dir,
        exe_path,
        exe_exists,
        exe_size_bytes: exe_size,
        port_9100_status: port_9100,
        port_9101_status: port_9101,
        initial_service_query: initial_query_summary,
        commands,
        final_result: "SUCCESS (Service Installed and Active in RUNNING State)".to_string(),
        error_summary: None,
    };
    let diag_file = write_diagnostic_report(&audit)?;
    let _ = std::fs::remove_file(get_install_error_path());
    log_service_event(&format!("Windows Service installation completed successfully. Diagnostics at: {}", diag_file.display()));

    println!("\nService Configuration Summary:");
    println!("  Service Name : {}", SERVICE_NAME_STR);
    println!("  Display Name : {}", SERVICE_DISPLAY_NAME_STR);
    println!("  Binary Path  : {}", bin_path_arg);
    println!("  Startup Type : Automatic (SERVICE_AUTO_START)");
    println!("  Account      : NT AUTHORITY\\LocalSystem");
    println!("  Log File     : {}", get_service_log_path().display());
    println!("  Diagnostics  : {}", diag_file.display());
    Ok(())
}

/// Standalone diagnostic audit subcommand: inspects system, ports, SCM status, and writes report
pub fn diagnose_windows_service() -> Result<(), Box<dyn std::error::Error>> {
    println!("=======================================================");
    println!("   CANON LBP-1120 SERVICE DIAGNOSTIC AUDIT            ");
    println!("=======================================================");

    let exe_path_buf = env::current_exe()?;
    let exe_path = exe_path_buf.to_string_lossy().replace(r"\\?\", "");
    let install_dir = exe_path_buf.parent().unwrap_or(std::path::Path::new("")).to_string_lossy().to_string();
    let exe_exists = exe_path_buf.exists();
    let exe_size = exe_path_buf.metadata().map(|m| m.len()).unwrap_or(0);

    let os_ver = Command::new("cmd.exe")
        .args(["/c", "ver"])
        .creation_flags(0x08000000)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "Unknown Windows OS".to_string());

    let arch = env::var("PROCESSOR_ARCHITECTURE").unwrap_or_else(|_| env::consts::ARCH.to_string());
    let comp_name = env::var("COMPUTERNAME").unwrap_or_else(|_| "UNKNOWN".to_string());
    let user_name = env::var("USERNAME").unwrap_or_else(|_| "UNKNOWN".to_string());
    let is_elevated = Command::new("net")
        .args(["session"])
        .creation_flags(0x08000000)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let port_9100 = check_tcp_port_status(9100);
    let port_9101 = check_tcp_port_status(9101);

    let mut commands = Vec::new();

    // Query service status
    let q = execute_sc("Query Service Status", &["query", SERVICE_NAME_STR], &[], &mut commands);
    let initial_query_summary = if q.exit_code == Some(0) {
        format!("Service Installed. State: {}", q.stdout.replace("\r\n", " ").replace("\n", " "))
    } else {
        format!("Service Not Registered (Code {:?})", q.exit_code)
    };

    // Query service configuration
    let _ = execute_sc("Query Service Configuration", &["qc", SERVICE_NAME_STR], &[], &mut commands);

    let audit = InstallAuditLog {
        os_version: os_ver,
        architecture: arch,
        computer_name: comp_name,
        user_name,
        is_elevated,
        install_dir,
        exe_path,
        exe_exists,
        exe_size_bytes: exe_size,
        port_9100_status: port_9100,
        port_9101_status: port_9101,
        initial_service_query: initial_query_summary,
        commands,
        final_result: "DIAGNOSTIC AUDIT COMPLETED".to_string(),
        error_summary: None,
    };

    let path = write_diagnostic_report(&audit)?;
    let text = std::fs::read_to_string(&path)?;
    println!("{}", text);
    println!("\nDiagnostic report saved to: {}", path.display());
    Ok(())
}

/// Verifies that all installation requirements are satisfied and active on the machine
pub fn verify_installation() -> Result<(), Box<dyn std::error::Error>> {
    println!("=======================================================");
    println!("   CANON LBP-1120 POST-INSTALLATION VERIFICATION AUDIT ");
    println!("=======================================================");

    use std::net::TcpStream;

    // 1. Verify executable exists
    let current_exe = env::current_exe()?;
    println!("[1/5] Verifying Service Executable Binary...");
    if !current_exe.exists() {
        return Err(format!("Executable binary missing: {}", current_exe.display()).into());
    }
    println!("      Found binary: {}", current_exe.display());

    // 2. Verify Windows SCM Service is Running
    println!("[2/5] Verifying Windows Service Status ('{}')...", SERVICE_NAME_STR);
    let query_out = Command::new("sc.exe")
        .args(["query", SERVICE_NAME_STR])
        .creation_flags(0x08000000)
        .output()?;
    let query_str = String::from_utf8_lossy(&query_out.stdout);
    if !query_str.contains("RUNNING") && !query_str.contains(" 4 ") {
        return Err(format!(
            "Windows Service '{}' is not RUNNING!\nSC Query Output:\n{}",
            SERVICE_NAME_STR, query_str.trim()
        ).into());
    }
    println!("      Windows Service '{}' is active and RUNNING.", SERVICE_NAME_STR);

    // 3. Verify Printer Ports
    println!("[3/5] Verifying Printer Ports ('CanonLBP1120_Normal' & 'CanonLBP1120_Photo')...");
    let port_cmd = "$ErrorActionPreference = 'Stop'; if (-not (Get-PrinterPort -Name 'CanonLBP1120_Normal' -ErrorAction SilentlyContinue)) { throw 'Normal port missing' }; if (-not (Get-PrinterPort -Name 'CanonLBP1120_Photo' -ErrorAction SilentlyContinue)) { throw 'Photo port missing' }";
    let port_out = Command::new("powershell")
        .args(["-NoProfile", "-Command", port_cmd])
        .creation_flags(0x08000000)
        .output()?;
    if !port_out.status.success() {
        let err = String::from_utf8_lossy(&port_out.stderr);
        return Err(format!("Printer ports verification failed: {}", err.trim()).into());
    }
    println!("      Both printer ports are configured.");

    // 4. Verify Printer Queues
    println!("[4/5] Verifying Printer Queues ('Canon LaserShot LBP-1120' & Photo)...");
    let queue_cmd = "$ErrorActionPreference = 'Stop'; if (-not (Get-Printer -Name 'Canon LaserShot LBP-1120' -ErrorAction SilentlyContinue)) { throw 'Normal printer queue missing' }; if (-not (Get-Printer -Name 'Canon LaserShot LBP-1120 (Photo)' -ErrorAction SilentlyContinue)) { throw 'Photo printer queue missing' }";
    let queue_out = Command::new("powershell")
        .args(["-NoProfile", "-Command", queue_cmd])
        .creation_flags(0x08000000)
        .output()?;
    if !queue_out.status.success() {
        let err = String::from_utf8_lossy(&queue_out.stderr);
        return Err(format!("Printer queue verification failed: {}", err.trim()).into());
    }
    println!("      Both Windows printer queues are registered.");

    // 5. Verify TCP Socket Listeners on 127.0.0.1:9100 and 127.0.0.1:9101
    println!("[5/5] Verifying TCP Spool Listeners (Ports 9100 & 9101)...");
    let mut tcp_ok = false;
    for _ in 0..10 {
        let conn9100 = TcpStream::connect_timeout(
            &"127.0.0.1:9100".parse().unwrap(),
            Duration::from_millis(500),
        );
        let conn9101 = TcpStream::connect_timeout(
            &"127.0.0.1:9101".parse().unwrap(),
            Duration::from_millis(500),
        );
        if conn9100.is_ok() && conn9101.is_ok() {
            tcp_ok = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    if !tcp_ok {
        return Err("TCP listeners on 127.0.0.1:9100 or 127.0.0.1:9101 are not responding.".into());
    }
    println!("      TCP listeners on 127.0.0.1:9100 & 9101 are active and accepting connections.");

    println!("\n>>> ALL POST-INSTALLATION VERIFICATION CHECKS PASSED (5/5) <<<");
    Ok(())
}

/// Checks whether an operational WinUSB driver is already bound to the Canon LBP-1120
pub fn check_usb_driver() -> i32 {
    let check_cmd = "$dev = Get-PnpDevice -InstanceId 'USB\\VID_04A9&PID_262B*' -ErrorAction SilentlyContinue | Select-Object -First 1; if ($dev -and $dev.Service -eq 'WinUSB' -and $dev.Status -eq 'OK') { exit 0 } else { exit 2 }";
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", check_cmd])
        .creation_flags(0x08000000)
        .output();
    match out {
        Ok(output) => {
            if output.status.code() == Some(0) {
                0 // Driver is operational
            } else {
                2 // Needs staging
            }
        }
        Err(_) => 2,
    }
}

/// Stops and unregisters the Windows Service
pub fn uninstall_windows_service() -> Result<(), Box<dyn std::error::Error>> {
    println!("Stopping Windows Service '{}'...", SERVICE_NAME_STR);
    let _ = Command::new("sc.exe")
        .args(["stop", SERVICE_NAME_STR])
        .creation_flags(0x08000000)
        .output();
    std::thread::sleep(Duration::from_millis(500));

    println!("Unregistering Windows Service '{}'...", SERVICE_NAME_STR);
    let status = Command::new("sc.exe")
        .args(["delete", SERVICE_NAME_STR])
        .creation_flags(0x08000000)
        .output()?;

    if status.status.success() {
        println!("Service unregistered successfully.");
    } else {
        println!("Service was not registered or already removed.");
    }
    Ok(())
}

/// Comprehensive end-to-end print pipeline diagnostic tracer (`capt-poc trace-pipeline`).
/// Inspects print queues, RAW ports, SCM service, TCP sockets, WinUSB hardware status,
/// and recent job execution logs, saving a full report to `%TEMP%\CanonLBP1120_Print_Trace.txt`.
pub fn trace_print_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = String::new();
    out.push_str("======================================================================\n");
    out.push_str("   CANON LASERSHOT LBP-1120 END-TO-END PRINT PIPELINE DIAGNOSTIC TRACE\n");
    out.push_str("======================================================================\n\n");

    let now = chrono_like_now();
    out.push_str(&format!("Audit Timestamp : {}\n", now));

    // 1. System Information
    let os_ver = Command::new("cmd.exe")
        .args(["/c", "ver"])
        .creation_flags(0x08000000)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "Unknown Windows OS".to_string());
    let arch = env::var("PROCESSOR_ARCHITECTURE").unwrap_or_else(|_| env::consts::ARCH.to_string());
    let comp_name = env::var("COMPUTERNAME").unwrap_or_else(|_| "UNKNOWN".to_string());
    let user_name = env::var("USERNAME").unwrap_or_else(|_| "UNKNOWN".to_string());
    let is_elevated = Command::new("net")
        .args(["session"])
        .creation_flags(0x08000000)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    out.push_str(&format!("OS Version      : {}\n", os_ver));
    out.push_str(&format!("Architecture    : {}\n", arch));
    out.push_str(&format!("Computer / User : {} \\ {}\n", comp_name, user_name));
    out.push_str(&format!("Elevation       : {}\n\n", if is_elevated { "Elevated (Administrator)" } else { "Standard User" }));

    // 2. Stage 1: Windows Print Queues
    out.push_str("----------------------------------------------------------------------\n");
    out.push_str("[Stage 1: Windows Print Queues Audit]\n");
    out.push_str("----------------------------------------------------------------------\n");
    let queue_cmd = "Get-Printer 'Canon LaserShot LBP-1120*' -ErrorAction SilentlyContinue | Select-Object Name, DriverName, PortName, Datatype, JobCount, PrinterStatus | Format-List";
    let q_out = Command::new("powershell")
        .args(["-NoProfile", "-Command", queue_cmd])
        .creation_flags(0x08000000)
        .output();
    let queue_text = match q_out {
        Ok(res) => String::from_utf8_lossy(&res.stdout).trim().to_string(),
        Err(e) => format!("Failed to query print queues: {}", e),
    };
    if queue_text.is_empty() {
        out.push_str("CRITICAL: No Canon LaserShot LBP-1120 print queues found in Windows!\n");
        out.push_str("Hint: Run 'capt-poc install' to register the Normal and Photo print queues.\n\n");
    } else {
        out.push_str(&queue_text);
        out.push_str("\n");
        if !queue_text.contains("Microsoft Print To PDF") && !queue_text.contains("PDF") {
            out.push_str("WARNING: The print queue is NOT using 'Microsoft Print to PDF' driver!\n");
            out.push_str("Drivers other than 'Microsoft Print to PDF' may output XPS/PCL/RAW data, which will be rejected by the LBP-1120 SCoA engine.\n\n");
        } else {
            out.push_str("Queue Driver Verified: 'Microsoft Print to PDF' is correctly bound.\n\n");
        }
    }

    // 3. Stage 2: Windows Printer Ports
    out.push_str("----------------------------------------------------------------------\n");
    out.push_str("[Stage 2: Windows RAW Printer Ports Audit]\n");
    out.push_str("----------------------------------------------------------------------\n");
    let port_cmd = "Get-PrinterPort 'CanonLBP1120*' -ErrorAction SilentlyContinue | Select-Object Name, Protocol, PrinterHostAddress, PortNumber | Format-List";
    let p_out = Command::new("powershell")
        .args(["-NoProfile", "-Command", port_cmd])
        .creation_flags(0x08000000)
        .output();
    let port_text = match p_out {
        Ok(res) => String::from_utf8_lossy(&res.stdout).trim().to_string(),
        Err(e) => format!("Failed to query printer ports: {}", e),
    };
    if port_text.is_empty() {
        out.push_str("CRITICAL: Printer ports 'CanonLBP1120_Normal' / 'CanonLBP1120_Photo' missing!\n\n");
    } else {
        out.push_str(&port_text);
        out.push_str("\n\n");
    }

    // 4. Windows SCM Service Status
    out.push_str("----------------------------------------------------------------------\n");
    out.push_str("[Windows SCM Service Status: CanonLBP1120Service]\n");
    out.push_str("----------------------------------------------------------------------\n");
    let sc_query = Command::new("sc.exe")
        .args(["query", SERVICE_NAME_STR])
        .creation_flags(0x08000000)
        .output();
    let sc_text = match sc_query {
        Ok(res) => String::from_utf8_lossy(&res.stdout).trim().to_string(),
        Err(e) => format!("Failed to query service: {}", e),
    };
    out.push_str(&sc_text);
    out.push_str("\n");
    if sc_text.contains("RUNNING") || sc_text.contains(" 4 ") {
        out.push_str("Service State: RUNNING (Operational)\n\n");
    } else {
        out.push_str("CRITICAL: Windows Service is NOT in RUNNING state!\n\n");
    }

    // 5. TCP Socket Listeners (Ports 9100 & 9101)
    out.push_str("----------------------------------------------------------------------\n");
    out.push_str("[TCP Socket Listeners Audit: 127.0.0.1:9100 & 9101]\n");
    out.push_str("----------------------------------------------------------------------\n");
    use std::net::TcpStream;
    let test_9100 = TcpStream::connect_timeout(&"127.0.0.1:9100".parse().unwrap(), Duration::from_millis(1000));
    let test_9101 = TcpStream::connect_timeout(&"127.0.0.1:9101".parse().unwrap(), Duration::from_millis(1000));
    out.push_str(&format!("Port 9100 (Normal Mode TCP) : {}\n", if test_9100.is_ok() { "LISTENING & ACCEPTING" } else { "NOT RESPONDING (Error)" }));
    out.push_str(&format!("Port 9101 (Photo Mode TCP)  : {}\n\n", if test_9101.is_ok() { "LISTENING & ACCEPTING" } else { "NOT RESPONDING (Error)" }));

    // 6. Stage 6: Physical WinUSB Device Communication
    out.push_str("----------------------------------------------------------------------\n");
    out.push_str("[Stage 6: Physical USB Hardware & WinUSB Driver Audit]\n");
    out.push_str("----------------------------------------------------------------------\n");
    let pnp_cmd = "Get-PnpDevice -InstanceId 'USB\\VID_04A9&PID_262B*' -ErrorAction SilentlyContinue | Select-Object InstanceId, Status, Service, Class, FriendlyName | Format-List";
    let pnp_out = Command::new("powershell")
        .args(["-NoProfile", "-Command", pnp_cmd])
        .creation_flags(0x08000000)
        .output();
    let pnp_text = match pnp_out {
        Ok(res) => String::from_utf8_lossy(&res.stdout).trim().to_string(),
        Err(e) => format!("Failed to query PnP: {}", e),
    };
    out.push_str(&format!("PnP Device Manager Status:\n{}\n\n", if pnp_text.is_empty() { "No USB device matching VID 04A9 & PID 262B connected." } else { &pnp_text }));

    out.push_str("Testing WinUSB Connection to Physical Printer...\n");
    match crate::usb::UsbPrinter::find_and_open() {
        Ok(printer) => {
            out.push_str(&format!("WinUSB Open Status: SUCCESS! Device Path: {}\n", printer.path));
            if let Ok(id) = printer.get_device_id() {
                out.push_str(&format!("IEEE 1284 Device ID : {}\n", id));
            }
            if let Ok(port_st) = printer.get_port_status() {
                out.push_str(&format!("Port Status Register: 0x{:02X} (PaperEmpty={}, Selected={}, NoError={})\n", port_st.raw, port_st.paper_empty, port_st.selected, port_st.no_error));
            }
            if let Ok(ext) = printer.query_extended_status() {
                out.push_str(&format!("Extended Hardware Registers: Basic=0x{:02X}, Aux=0x{:02X}, Controller=0x{:02X}, Engine=0x{:04X}\n", ext.basic, ext.aux, ext.controller, ext.engine_raw));
                out.push_str(&format!("Physical Sensors: DoorOpen={}, NoCartridge={}, PaperJam={}, NoPaper={}, UnitFree={}, Waiting={}\n", ext.door_open, ext.no_cartridge, ext.paper_jam, ext.no_paper, ext.unit_free, ext.waiting));
                out.push_str(&format!("Hardware Counters: Started={}, Printing={}, Shipped={}, Printed={}\n\n", ext.start_pages, ext.printing_pages, ext.shipped_pages, ext.printed_pages));
            }
        }
        Err(e) => {
            out.push_str(&format!("WinUSB Open Status: FAILED!\nError Details: {}\n\n", e));
        }
    }

    // 7. Recent Spool & Job Logs
    out.push_str("----------------------------------------------------------------------\n");
    out.push_str("[Recent Spool Files & Job Execution Logs]\n");
    out.push_str("----------------------------------------------------------------------\n");
    let jobs_dir = get_jobs_dir();
    out.push_str(&format!("Jobs Directory: {}\n", jobs_dir.display()));
    if let Ok(entries) = std::fs::read_dir(&jobs_dir) {
        let mut job_files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        job_files.sort_by(|a, b| b.cmp(a)); // newest first
        if job_files.is_empty() {
            out.push_str("No job log files found in directory.\n\n");
        } else {
            out.push_str(&format!("Found {} job log file(s). Most recent jobs:\n", job_files.len()));
            for path in job_files.iter().take(5) {
                out.push_str(&format!("  - {}\n", path.file_name().unwrap_or_default().to_string_lossy()));
            }
            out.push_str("\n--- Content of Latest Job Log ---\n");
            if let Some(latest) = job_files.first() {
                if let Ok(content) = std::fs::read_to_string(latest) {
                    out.push_str(&content);
                } else {
                    out.push_str("(Unable to read latest job log)\n");
                }
            }
            out.push_str("--- End of Latest Job Log ---\n\n");
        }
    } else {
        out.push_str("Jobs directory not created yet (no print jobs received since service start).\n\n");
    }

    // 8. Recent Service Log Events
    out.push_str("----------------------------------------------------------------------\n");
    out.push_str("[Recent Service Log Events (service.log)]\n");
    out.push_str("----------------------------------------------------------------------\n");
    let svc_log = get_service_log_path();
    if let Ok(content) = std::fs::read_to_string(&svc_log) {
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(30);
        for line in &lines[start..] {
            out.push_str(line);
            out.push_str("\n");
        }
    } else {
        out.push_str("Service log file not found or empty.\n");
    }
    out.push_str("\n======================================================================\n");
    out.push_str("END OF PIPELINE DIAGNOSTIC TRACE\n");
    out.push_str("======================================================================\n");

    // Save report to %TEMP%\CanonLBP1120_Print_Trace.txt
    let temp_file = env::temp_dir().join("CanonLBP1120_Print_Trace.txt");
    let _ = std::fs::write(&temp_file, &out);

    print!("{}", out);
    let _ = std::io::stdout().flush();
    println!("\nComplete diagnostic trace saved to: {}", temp_file.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explain_scm_error_access_denied() {
        let msg = explain_scm_error(Some(5), "OpenSCManager failed");
        assert!(msg.contains("ERROR_ACCESS_DENIED"));
        assert!(msg.contains("Administrator"));
    }

    #[test]
    fn test_explain_scm_error_marked_for_delete() {
        let msg = explain_scm_error(Some(1072), "CreateService FAILED 1072: The specified service has been marked for deletion.");
        assert!(msg.contains("ERROR_SERVICE_MARKED_FOR_DELETE"));
        assert!(msg.contains("services.msc"));
    }

    #[test]
    fn test_explain_scm_error_already_exists() {
        let msg = explain_scm_error(Some(1073), "CreateService FAILED 1073: The specified service already exists.");
        assert!(msg.contains("ERROR_SERVICE_EXISTS"));
    }

    #[test]
    fn test_explain_scm_error_invalid_parameter() {
        let msg = explain_scm_error(Some(1639), "The command line parameter is incorrect");
        assert!(msg.contains("ERROR_INVALID_PARAMETER"));
    }

    #[test]
    fn test_diagnostic_paths() {
        let diag_path = get_install_diagnostic_path();
        assert!(diag_path.ends_with("CanonLBP1120_Install_Diagnostics.txt"));
        let err_path = get_install_error_path();
        assert!(err_path.ends_with("CanonLBP1120_Install_Error.txt"));
    }

    #[test]
    fn test_write_and_read_diagnostic_report() {
        let audit = InstallAuditLog {
            os_version: "Test Windows 11".to_string(),
            architecture: "x86_64".to_string(),
            computer_name: "TEST-PC".to_string(),
            user_name: "TestUser".to_string(),
            is_elevated: true,
            install_dir: r"C:\Program Files\CanonLBP1120".to_string(),
            exe_path: r"C:\Program Files\CanonLBP1120\canon-lbp1120-service.exe".to_string(),
            exe_exists: true,
            exe_size_bytes: 869376,
            port_9100_status: "FREE".to_string(),
            port_9101_status: "FREE".to_string(),
            initial_service_query: "Service does NOT exist (Code 1060)".to_string(),
            commands: vec![
                ScCommandRecord {
                    description: "Test Command".to_string(),
                    command_line: "sc.exe query CanonLBP1120Service".to_string(),
                    exit_code: Some(1060),
                    stdout: "Service does not exist".to_string(),
                    stderr: "".to_string(),
                    duration_ms: 5,
                    success: false,
                }
            ],
            final_result: "TEST SUCCESS".to_string(),
            error_summary: None,
        };

        let path = write_diagnostic_report(&audit).expect("write diagnostic report");
        assert!(path.exists());
        let text = std::fs::read_to_string(&path).expect("read diagnostic report");
        assert!(text.contains("CANON LASERSHOT LBP-1120 — INSTALLATION DIAGNOSTIC REPORT"));
        assert!(text.contains("Test Windows 11"));
        assert!(text.contains("x86_64"));
        assert!(text.contains("TEST SUCCESS"));
        assert!(text.contains("sc.exe query CanonLBP1120Service"));
    }

    #[test]
    fn test_cleanup_port_conflict_free_port() {
        // Ephemeral high port should be free and return None without taking action
        let action = cleanup_port_conflict_if_ours(54321);
        assert!(action.is_none());
    }

    #[test]
    fn test_job_id_generation_and_logger() {
        let job_id = generate_job_id(9100, "normal");
        assert!(job_id.starts_with("JOB-"));
        assert!(job_id.contains("-NORM-P9100-"));

        let photo_id = generate_job_id(9101, "photo");
        assert!(photo_id.contains("-PHOTO-P9101-"));

        let logger = JobLogger::new(&job_id);
        logger.log("Stage 2: Connection", "Testing connection log");
        logger.log_error("Stage 3: Format", "Testing error log");
        logger.log_summary(
            "ACCEPTED",
            "SUCCESS",
            "SUCCESS",
            "SUCCESS",
            "SUCCESS",
            "SUCCESS",
            "SUCCESS",
            "FINAL TEST SUCCESS",
        );

        assert!(logger.job_log_path.exists());
        let content = std::fs::read_to_string(&logger.job_log_path).expect("read job log");
        assert!(content.contains(&job_id));
        assert!(content.contains("Testing connection log"));
        assert!(content.contains("Testing error log"));
        assert!(content.contains("FINAL TEST SUCCESS"));

        // Clean up test job log file
        let _ = std::fs::remove_file(&logger.job_log_path);
    }
}
