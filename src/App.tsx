import React, { useState, useEffect, useCallback } from 'react';
import {
  Printer,
  Sliders,
  Binary,
  Layers,
  Sun,
  Moon,
  Play,
  Usb,
  Volume2,
  CheckCircle2,
  AlertTriangle,
} from 'lucide-react';
import {
  advanceJobState,
  type HalftoneAlgorithmId,
  type PaperSizeCode,
  type PresetId,
  type PrintJobItem,
} from './core/captEngine';
import { DashboardTab } from './components/DashboardTab';
import { HalftoneLabTab } from './components/HalftoneLabTab';
import { ProtocolReLabTab, type RepositoryAuditData } from './components/ProtocolReLabTab';
import { ArchitectureDriversTab } from './components/ArchitectureDriversTab';

type ActiveTab = 'dashboard' | 'halftone' | 'protocol' | 'architecture';

const INITIAL_JOBS: PrintJobItem[] = [
  {
    id: 'JOB-1041',
    documentName: 'Canon_LBP1120_Protocol_Spec_v2.pdf',
    sourceApp: 'Microsoft Print to PDF (TCP :9100)',
    pages: 2,
    preset: 'mixed_document',
    algorithm: 'normal_clustered_8x8',
    stage: 'Completed',
    stageIndex: 6,
    submittedAt: '09:14:22',
    scoaBytes: 94_820,
    usbPacketsSent: 3,
    usbPacketsTotal: 3,
    baselineShippedCounter: 12,
    currentShippedCounter: 14,
    physicalDeliveryConfirmed: true,
    transitionLog: [
      { stage: 'Submitted', timestamp: '09:14:22', detail: 'Received 2-page PDF stream on TCP Port 9100.' },
      { stage: 'Accepted', timestamp: '09:14:22', detail: 'Spooler stream validated (%PDF magic & geometry verified).' },
      { stage: 'Rendering', timestamp: '09:14:23', detail: 'Rasterizing 2 page(s) at 600 DPI (4736×6778 px canvas).' },
      { stage: 'Processing', timestamp: '09:14:23', detail: 'Applying Normal Snapped + 8×8 Clustered Dot + SCoA 2D delta encoding (92.6 KB).' },
      { stage: 'Transmitting', timestamp: '09:14:24', detail: 'PCR_GO_ONLINE (0xADEADBEE) -> Streaming 3 IC_VIDEO_DATA packets on Bulk OUT 0x01.' },
      { stage: 'Printing', timestamp: '09:14:26', detail: 'USB transfer finished (94820 B). Transmitted IC_END_PAGE (0xD0A2); waiting for physical fuser & exit sensor confirmation...' },
      { stage: 'Completed', timestamp: '09:14:32', detail: 'Hardware shipped_pages counter advanced (12 -> 14). Physical paper delivery confirmed; issued PCR_GO_OFFLINE.' },
    ],
  },
  {
    id: 'JOB-1042',
    documentName: 'PCB_TopCopper_600DPI_Schematic.pdf',
    sourceApp: 'KiCad 8.0 Plotter',
    pages: 1,
    preset: 'graphics',
    algorithm: 'canon_cap3ht_16x16',
    stage: 'Printing',
    stageIndex: 5,
    submittedAt: '09:18:05',
    scoaBytes: 62_410,
    usbPacketsSent: 2,
    usbPacketsTotal: 2,
    baselineShippedCounter: 14,
    currentShippedCounter: 14,
    physicalDeliveryConfirmed: false,
    transitionLog: [
      { stage: 'Submitted', timestamp: '09:18:05', detail: 'Received 1-page vector schematic on Named Pipe.' },
      { stage: 'Accepted', timestamp: '09:18:05', detail: 'Spooler stream validated (%PDF magic & geometry verified).' },
      { stage: 'Rendering', timestamp: '09:18:05', detail: 'Rasterizing 1 page(s) at 600 DPI (4736×6778 px canvas).' },
      { stage: 'Processing', timestamp: '09:18:06', detail: 'Applying Canon Cap3ht.bin 16×16 Supercell + SCoA 2D delta encoding (60.9 KB).' },
      { stage: 'Transmitting', timestamp: '09:18:06', detail: 'PCR_GO_ONLINE (0xADEADBEE) -> Streaming 2 IC_VIDEO_DATA packets on Bulk OUT 0x01.' },
      { stage: 'Printing', timestamp: '09:18:07', detail: 'USB transfer finished (62410 B). Transmitted IC_END_PAGE (0xD0A2); waiting for physical fuser & exit sensor confirmation...' },
    ],
  },
];

export function App() {
  const [activeTab, setActiveTab] = useState<ActiveTab>('dashboard');
  const [darkMode, setDarkMode] = useState<boolean>(true);
  const [auditData, setAuditData] = useState<RepositoryAuditData | null>(null);
  const [activeSoundFile, setActiveSoundFile] = useState<string | null>('Cap3strt.wav');
  const [shippedPagesCounter, setShippedPagesCounter] = useState<number>(14);
  const [jobs, setJobs] = useState<PrintJobItem[]>(INITIAL_JOBS);

  const [printerFault, setPrinterFault] = useState({
    doorOpen: false,
    paperEmpty: false,
    paperJam: false,
    noCartridge: false,
    usbConnected: true,
  });

  useEffect(() => {
    if (darkMode) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  }, [darkMode]);

  useEffect(() => {
    fetch('/api/repository-audit')
      .then((res) => res.json())
      .then((data: RepositoryAuditData) => {
        setAuditData(data);
      })
      .catch(() => {
        // Fallback if endpoint is still warming up
      });
  }, []);

  const handlePlayCanonSound = useCallback((filename: string) => {
    setActiveSoundFile(filename);
    try {
      const audio = new Audio(`/api/sounds/${encodeURIComponent(filename)}`);
      audio.volume = 0.65;
      audio.play().catch(() => {
        // Ignore headless browser autoplay restrictions
      });
    } catch {
      // Ignore audio errors
    }
  }, []);

  const handleToggleFault = useCallback(
    (key: keyof typeof printerFault) => {
      setPrinterFault((prev) => {
        const nextVal = !prev[key];
        const nextState = { ...prev, [key]: nextVal };
        if (nextVal && key !== 'usbConnected') {
          const soundMap: Record<string, string> = {
            doorOpen: 'Cap3covr.wav',
            paperEmpty: 'Cap3addp.wav',
            paperJam: 'Cap3jam.wav',
            noCartridge: 'Cap3cart.wav',
          };
          handlePlayCanonSound(soundMap[key] || 'Cap3err.wav');
        } else if (key === 'usbConnected' && !nextVal) {
          handlePlayCanonSound('Cap3err.wav');
        }
        return nextState;
      });
    },
    [handlePlayCanonSound]
  );

  const handleRunHardwareRecovery = useCallback(() => {
    setPrinterFault({
      doorOpen: false,
      paperEmpty: false,
      paperJam: false,
      noCartridge: false,
      usbConnected: true,
    });
    handlePlayCanonSound('Cap3resm.wav');
  }, [handlePlayCanonSound]);

  const handleStepJob = useCallback(
    (jobId: string) => {
      setJobs((prevJobs) =>
        prevJobs.map((job) => {
          if (job.id !== jobId) return job;
          const { updatedJob, newShippedCounter, soundAlert } = advanceJobState(
            job,
            printerFault,
            shippedPagesCounter
          );
          if (newShippedCounter !== shippedPagesCounter) {
            setShippedPagesCounter(newShippedCounter);
          }
          if (soundAlert) {
            handlePlayCanonSound(soundAlert);
          }
          return updatedJob;
        })
      );
    },
    [handlePlayCanonSound, printerFault, shippedPagesCounter]
  );

  const handleAutoCompleteJob = useCallback(
    (jobId: string) => {
      let stepCount = 0;
      const timer = setInterval(() => {
        stepCount++;
        setJobs((prevJobs) => {
          const target = prevJobs.find((j) => j.id === jobId);
          if (
            !target ||
            target.stage === 'Completed' ||
            target.stage === 'Failed' ||
            target.stage === 'Cancelled' ||
            stepCount > 8
          ) {
            clearInterval(timer);
            return prevJobs;
          }
          return prevJobs.map((j) => {
            if (j.id !== jobId) return j;
            const res = advanceJobState(j, printerFault, shippedPagesCounter);
            if (res.newShippedCounter !== shippedPagesCounter) {
              setShippedPagesCounter(res.newShippedCounter);
            }
            if (res.soundAlert) {
              handlePlayCanonSound(res.soundAlert);
            }
            return res.updatedJob;
          });
        });
      }, 240);
    },
    [handlePlayCanonSound, printerFault, shippedPagesCounter]
  );

  const handleRetryJob = useCallback(
    (jobId: string) => {
      const nowStr = new Date().toLocaleTimeString('en-US', { hour12: false });
      setJobs((prev) =>
        prev.map((j) => {
          if (j.id !== jobId) return j;
          return {
            ...j,
            stage: 'Submitted',
            stageIndex: 0,
            usbPacketsSent: 0,
            physicalDeliveryConfirmed: false,
            errorReason: undefined,
            baselineShippedCounter: shippedPagesCounter,
            currentShippedCounter: shippedPagesCounter,
            transitionLog: [
              ...j.transitionLog,
              {
                stage: 'Submitted',
                timestamp: nowStr,
                detail: 'Operator triggered recovery & job retry from clean state.',
              },
            ],
          };
        })
      );
      handlePlayCanonSound('Cap3resm.wav');
    },
    [handlePlayCanonSound, shippedPagesCounter]
  );

  const handleCancelJob = useCallback(
    (jobId: string) => {
      const nowStr = new Date().toLocaleTimeString('en-US', { hour12: false });
      setJobs((prev) =>
        prev.map((j) => {
          if (j.id !== jobId) return j;
          return {
            ...j,
            stage: 'Cancelled',
            errorReason: 'Cancelled by operator (PCR_DISCARD_DATA 0xE0A4 -> PCR_GO_OFFLINE 0xE0A6).',
            transitionLog: [
              ...j.transitionLog,
              {
                stage: 'Cancelled',
                timestamp: nowStr,
                detail: 'Transmitted PCR_DISCARD_DATA (0xE0A4) & released engine.',
              },
            ],
          };
        })
      );
      handlePlayCanonSound('Cap3stop.wav');
    },
    [handlePlayCanonSound]
  );

  const handleClearFinishedJobs = useCallback(() => {
    setJobs((prev) =>
      prev.filter((j) => j.stage !== 'Completed' && j.stage !== 'Cancelled')
    );
  }, []);

  const handleSubmitNewJob = useCallback(
    (params: {
      documentName: string;
      sourceApp: string;
      pages: number;
      preset: PresetId;
      algorithm: HalftoneAlgorithmId;
      paperSize: PaperSizeCode;
      dpi: 600 | 300;
      tonerDensity: number;
      smoothEnable: boolean;
      tonerSaving: boolean;
    }) => {
      const nowStr = new Date().toLocaleTimeString('en-US', { hour12: false });
      const newId = `JOB-${1043 + jobs.length}`;
      const scoaBytes = params.pages * (params.tonerSaving ? 34_200 : 58_400);
      const packetsTotal = Math.max(1, Math.ceil(scoaBytes / 49_152));
      const newJob: PrintJobItem = {
        id: newId,
        documentName: params.documentName,
        sourceApp: `${params.sourceApp} (${params.paperSize} @ ${params.dpi} DPI)`,
        pages: params.pages,
        preset: params.preset,
        algorithm: params.algorithm,
        stage: 'Submitted',
        stageIndex: 0,
        submittedAt: nowStr,
        scoaBytes,
        usbPacketsSent: 0,
        usbPacketsTotal: packetsTotal,
        baselineShippedCounter: shippedPagesCounter,
        currentShippedCounter: shippedPagesCounter,
        physicalDeliveryConfirmed: false,
        transitionLog: [
          {
            stage: 'Submitted',
            timestamp: nowStr,
            detail: `Queued ${params.pages} page(s) (${params.paperSize}, density=${params.tonerDensity}, SCoA=${(scoaBytes / 1024).toFixed(1)} KB).`,
          },
        ],
      };
      setJobs((prev) => [newJob, ...prev]);
      handlePlayCanonSound('Cap3strt.wav');
    },
    [handlePlayCanonSound, jobs.length, shippedPagesCounter]
  );

  const isHardwareHealthy =
    printerFault.usbConnected &&
    !printerFault.doorOpen &&
    !printerFault.paperEmpty &&
    !printerFault.paperJam &&
    !printerFault.noCartridge;

  return (
    <div className="min-h-screen bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100 flex flex-col">
      {/* Top Bar Contract: Zone 1 (Wordmark) | Zone 2 (4 Single-line Nav Items) | Zone 3 (1 Primary CTA) */}
      <header className="h-14 border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 px-6 flex items-center justify-between gap-6 shrink-0">
        <div className="flex items-center gap-2.5 whitespace-nowrap shrink-0">
          <Printer className="w-5 h-5 text-blue-600 dark:text-blue-400 shrink-0" />
          <span className="font-semibold text-base tracking-tight text-slate-900 dark:text-slate-100 whitespace-nowrap shrink-0">
            Canon LBP-1120 CAPT Suite
          </span>
        </div>

        <nav className="flex items-center gap-1 overflow-x-auto">
          <button
            onClick={() => setActiveTab('dashboard')}
            className={`px-3 py-1.5 rounded-md text-sm font-medium whitespace-nowrap shrink-0 transition-colors cursor-pointer flex items-center gap-1.5 ${
              activeTab === 'dashboard'
                ? 'bg-slate-100 dark:bg-slate-800 text-blue-600 dark:text-blue-400'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
            }`}
          >
            <Printer className="w-4 h-4" />
            Overview & Queue
          </button>
          <button
            onClick={() => setActiveTab('halftone')}
            className={`px-3 py-1.5 rounded-md text-sm font-medium whitespace-nowrap shrink-0 transition-colors cursor-pointer flex items-center gap-1.5 ${
              activeTab === 'halftone'
                ? 'bg-slate-100 dark:bg-slate-800 text-blue-600 dark:text-blue-400'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
            }`}
          >
            <Sliders className="w-4 h-4" />
            Halftone Studio
          </button>
          <button
            onClick={() => setActiveTab('protocol')}
            className={`px-3 py-1.5 rounded-md text-sm font-medium whitespace-nowrap shrink-0 transition-colors cursor-pointer flex items-center gap-1.5 ${
              activeTab === 'protocol'
                ? 'bg-slate-100 dark:bg-slate-800 text-blue-600 dark:text-blue-400'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
            }`}
          >
            <Binary className="w-4 h-4" />
            CAPT Protocol Lab
          </button>
          <button
            onClick={() => setActiveTab('architecture')}
            className={`px-3 py-1.5 rounded-md text-sm font-medium whitespace-nowrap shrink-0 transition-colors cursor-pointer flex items-center gap-1.5 ${
              activeTab === 'architecture'
                ? 'bg-slate-100 dark:bg-slate-800 text-blue-600 dark:text-blue-400'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
            }`}
          >
            <Layers className="w-4 h-4" />
            Architecture & Drivers
          </button>
        </nav>

        <button
          onClick={() => {
            setActiveTab('dashboard');
            handleSubmitNewJob({
              documentName: 'Canon_LBP1120_SelfTest_600DPI.pdf',
              sourceApp: 'CAPT Diagnostic Suite',
              pages: 1,
              preset: 'mixed_document',
              algorithm: 'canon_cap3ht_16x16',
              paperSize: 'A4',
              dpi: 600,
              tonerDensity: 31,
              smoothEnable: true,
              tonerSaving: false,
            });
          }}
          className="px-3.5 py-1.5 rounded-md text-sm font-medium bg-blue-600 hover:bg-blue-500 text-white whitespace-nowrap shrink-0 transition-colors cursor-pointer inline-flex items-center gap-1.5 shadow-xs"
        >
          <Play className="w-3.5 h-3.5 fill-current" />
          Quick Test Print
        </button>
      </header>

      {/* Secondary Hardware Context Bar */}
      <div className="border-b border-slate-200 dark:border-slate-800 bg-slate-100/70 dark:bg-slate-900/60 px-6 py-2 flex flex-wrap items-center justify-between gap-4 text-xs text-slate-600 dark:text-slate-400">
        <div className="flex flex-wrap items-center gap-2 font-mono">
          <span className="inline-flex items-center gap-1.5 font-medium text-slate-800 dark:text-slate-200">
            <Usb className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" />
            USB 04A9:262B
          </span>
          <span>·</span>
          <span>CAPT 1.0 / SCoA Engine</span>
          <span>·</span>
          <span className="inline-flex items-center gap-1">
            {isHardwareHealthy ? (
              <>
                <CheckCircle2 className="w-3.5 h-3.5 text-emerald-500" />
                <span className="text-emerald-600 dark:text-emerald-400 font-medium">
                  ONLINE & READY
                </span>
              </>
            ) : (
              <>
                <AlertTriangle className="w-3.5 h-3.5 text-amber-500" />
                <span className="text-amber-600 dark:text-amber-400 font-medium">
                  HARDWARE INTERLOCK ACTIVE
                </span>
              </>
            )}
          </span>
          <span>·</span>
          <span>512 KB Onboard FIFO (480 KB Governor)</span>
          <span>·</span>
          <span>Shipped Pages: {shippedPagesCounter}</span>
        </div>

        <div className="flex items-center gap-4">
          <div className="flex items-center gap-1.5 font-mono text-[11px] text-slate-500 dark:text-slate-400">
            <Volume2 className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" />
            <span>Last Alert: {activeSoundFile || 'None'}</span>
          </div>
          <button
            onClick={() => setDarkMode((d) => !d)}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 transition-colors cursor-pointer"
            title="Toggle Light / Dark Theme"
          >
            {darkMode ? (
              <>
                <Sun className="w-3.5 h-3.5 text-amber-400" />
                <span>Light</span>
              </>
            ) : (
              <>
                <Moon className="w-3.5 h-3.5 text-slate-700" />
                <span>Dark</span>
              </>
            )}
          </button>
        </div>
      </div>

      {/* Main Workspace Container */}
      <main className="flex-1 max-w-[1600px] w-full mx-auto px-6 py-6">
        {activeTab === 'dashboard' && (
          <DashboardTab
            jobs={jobs}
            shippedPagesCounter={shippedPagesCounter}
            printerFault={printerFault}
            onToggleFault={handleToggleFault}
            onRunHardwareRecovery={handleRunHardwareRecovery}
            onStepJob={handleStepJob}
            onAutoCompleteJob={handleAutoCompleteJob}
            onRetryJob={handleRetryJob}
            onCancelJob={handleCancelJob}
            onClearFinishedJobs={handleClearFinishedJobs}
            onSubmitNewJob={handleSubmitNewJob}
            onPlayCanonSound={handlePlayCanonSound}
            activeSoundFile={activeSoundFile}
            availableSounds={auditData?.sounds || []}
          />
        )}

        {activeTab === 'halftone' && <HalftoneLabTab />}

        {activeTab === 'protocol' && <ProtocolReLabTab auditData={auditData} />}

        {activeTab === 'architecture' && (
          <ArchitectureDriversTab sourceFiles={auditData?.sourceFiles || []} />
        )}
      </main>
    </div>
  );
}

export default App;
