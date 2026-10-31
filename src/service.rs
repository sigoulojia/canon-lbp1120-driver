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

/// Returns the path to the service log file: `%ProgramData%\CanonLBP1120\service.log`
pub fn get_service_log_path() -> PathBuf {
    let program_data = env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
    let mut dir = PathBuf::from(program_data);
    dir.push("CanonLBP1120");
    dir.push("service.log");
    dir
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

/// Registers the binary as a native Windows Service using Service Control Manager (`sc.exe`)
pub fn install_windows_service() -> Result<(), Box<dyn std::error::Error>> {
    println!("=======================================================");
    println!("   CANON LBP-1120 WINDOWS SERVICE INSTALLER           ");
    println!("=======================================================");

    let exe_path = env::current_exe()?.to_string_lossy().replace(r"\\?\", "");
    let bin_path_arg = format!("\"{}\" service-run", exe_path);

    println!("[1/4] Stopping existing service instance if running...");
    let _ = Command::new("sc.exe")
        .args(["stop", SERVICE_NAME_STR])
        .creation_flags(0x08000000)
        .output();
    std::thread::sleep(Duration::from_millis(800));

    println!("[2/4] Registering Windows Service '{}'...", SERVICE_NAME_STR);
    // Delete previous definition if exists
    let _ = Command::new("sc.exe")
        .args(["delete", SERVICE_NAME_STR])
        .creation_flags(0x08000000)
        .output();
    std::thread::sleep(Duration::from_millis(500));

    // Create service
    let create_status = Command::new("sc.exe")
        .args([
            "create",
            SERVICE_NAME_STR,
            &format!("binPath= {}", bin_path_arg),
            &format!("DisplayName= {}", SERVICE_DISPLAY_NAME_STR),
            "start= auto",
            "obj= LocalSystem",
        ])
        .creation_flags(0x08000000)
        .output()?;

    if !create_status.status.success() {
        let stdout_msg = String::from_utf8_lossy(&create_status.stdout);
        let stderr_msg = String::from_utf8_lossy(&create_status.stderr);
        let combined = format!("{}\n{}", stdout_msg.trim(), stderr_msg.trim());
        return Err(format!("Failed to create Windows service: {}", combined.trim()).into());
    }

    // Set description
    let _ = Command::new("sc.exe")
        .args([
            "description",
            SERVICE_NAME_STR,
            "Canon LaserShot LBP-1120 native driver service providing 600 DPI rendering, SCoA compression, and WinUSB transmission.",
        ])
        .creation_flags(0x08000000)
        .output();

    // Set crash recovery actions (restart after 5s on 1st failure, 10s on 2nd)
    println!("[3/4] Configuring automatic crash recovery policy...");
    let _ = Command::new("sc.exe")
        .args([
            "failure",
            SERVICE_NAME_STR,
            "reset= 86400",
            "actions= restart/5000/restart/10000/none/0",
        ])
        .creation_flags(0x08000000)
        .output();

    println!("[4/4] Starting service '{}' and waiting for RUNNING state...", SERVICE_NAME_STR);
    let _ = Command::new("sc.exe")
        .args(["start", SERVICE_NAME_STR])
        .creation_flags(0x08000000)
        .output();

    // Poll service status until RUNNING (up to 10 seconds)
    let mut running = false;
    let mut last_status = String::new();
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(500));
        let query_status = Command::new("sc.exe")
            .args(["query", SERVICE_NAME_STR])
            .creation_flags(0x08000000)
            .output();
        if let Ok(out) = query_status {
            last_status = String::from_utf8_lossy(&out.stdout).to_string();
            if last_status.contains("RUNNING") || last_status.contains(" 4 ") {
                running = true;
                break;
            }
        }
    }

    if !running {
        return Err(format!(
            "Service failed to reach RUNNING state within 10 seconds.\nLast query output:\n{}",
            last_status.trim()
        ).into());
    }

    println!("Service started and confirmed in RUNNING state!");
    println!("\nService Configuration Summary:");
    println!("  Service Name : {}", SERVICE_NAME_STR);
    println!("  Display Name : {}", SERVICE_DISPLAY_NAME_STR);
    println!("  Binary Path  : {}", bin_path_arg);
    println!("  Startup Type : Automatic (SERVICE_AUTO_START)");
    println!("  Account      : NT AUTHORITY\\LocalSystem");
    println!("  Log File     : {}", get_service_log_path().display());
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
