import React, { useState } from 'react';
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Download,
  FileText,
  Play,
  Printer,
  RefreshCw,
  RotateCcw,
  ShieldAlert,
  Sliders,
  Trash2,
  Volume2,
  XCircle,
} from 'lucide-react';
import {
  ALGORITHM_LABELS,
  HALFTONE_PRESETS,
  JOB_STAGE_SEQUENCE,
  LBP1120_FIFO_RAM_BYTES,
  LBP1120_SAFE_GOVERNOR_BYTES,
  getPaperGeometry,
  type HalftoneAlgorithmId,
  type PaperSizeCode,
  type PresetId,
  type PrintJobItem,
} from '../core/captEngine';

interface DashboardTabProps {
  jobs: PrintJobItem[];
  shippedPagesCounter: number;
  printerFault: {
    doorOpen: boolean;
    paperEmpty: boolean;
    paperJam: boolean;
    noCartridge: boolean;
    usbConnected: boolean;
  };
  onToggleFault: (key: keyof DashboardTabProps['printerFault']) => void;
  onRunHardwareRecovery: () => void;
  onStepJob: (jobId: string) => void;
  onAutoCompleteJob: (jobId: string) => void;
  onRetryJob: (jobId: string) => void;
  onCancelJob: (jobId: string) => void;
  onClearFinishedJobs: () => void;
  onSubmitNewJob: (job: {
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
  }) => void;
  onPlayCanonSound: (filename: string) => void;
  activeSoundFile: string | null;
  availableSounds: { filename: string; label: string; sizeBytes: number }[];
}

export const DashboardTab: React.FC<DashboardTabProps> = ({
  jobs,
  shippedPagesCounter,
  printerFault,
  onToggleFault,
  onRunHardwareRecovery,
  onStepJob,
  onAutoCompleteJob,
  onRetryJob,
  onCancelJob,
  onClearFinishedJobs,
  onSubmitNewJob,
  onPlayCanonSound,
  activeSoundFile,
  availableSounds,
}) => {
  const [docName, setDocName] = useState('Q4_Architecture_Blueprint.pdf');
  const [sourceApp, setSourceApp] = useState('WinRT PDF Spooler (TCP 9100)');
  const [pageCount, setPageCount] = useState(3);
  const [selectedPreset, setSelectedPreset] = useState<PresetId>('mixed_document');
  const [paperSize, setPaperSize] = useState<PaperSizeCode>('A4');
  const [dpi, setDpi] = useState<600 | 300>(600);
  const [orientation, setOrientation] = useState<'Portrait' | 'Landscape'>('Portrait');
  const [tonerDensity, setTonerDensity] = useState(31);
  const [smoothEnable, setSmoothEnable] = useState(true);
  const [tonerSaving, setTonerSaving] = useState(false);
  const [selectedJobId, setSelectedJobId] = useState<string>(jobs[0]?.id || '');
  const [formError, setFormError] = useState<string | null>(null);

  const activePresetObj =
    HALFTONE_PRESETS.find((p) => p.id === selectedPreset) || HALFTONE_PRESETS[1];
  const paperGeom = getPaperGeometry(paperSize, dpi);

  const anyFault =
    !printerFault.usbConnected ||
    printerFault.doorOpen ||
    printerFault.paperEmpty ||
    printerFault.paperJam ||
    printerFault.noCartridge;

  const activeJob = jobs.find(
    (j) =>
      j.stage !== 'Completed' && j.stage !== 'Failed' && j.stage !== 'Cancelled'
  );

  // Compute simulated register values from live state
  const basicReg = !printerFault.usbConnected
    ? 0xff
    : anyFault
    ? 0x92
    : activeJob
    ? 0x01
    : 0x40;
  const auxReg = activeJob?.stage === 'Printing' ? 0x06 : activeJob ? 0x02 : 0x00;
  const engineReg =
    (printerFault.doorOpen ? 0x4000 : 0) |
    (printerFault.noCartridge ? 0x2000 : 0) |
    (printerFault.paperEmpty ? 0x0200 : 0) |
    (printerFault.paperJam ? 0x0100 : 0);
  const paperReg = printerFault.paperEmpty ? 0x00 : 0x80;

  const currentFifoBytes = activeJob ? activeJob.scoaBytes : 38_420;
  const fifoPercent = Math.min(
    100,
    Number(((currentFifoBytes / LBP1120_FIFO_RAM_BYTES) * 100).toFixed(1))
  );

  const handlePresetChange = (presetId: PresetId) => {
    setSelectedPreset(presetId);
    const p = HALFTONE_PRESETS.find((item) => item.id === presetId);
    if (p) {
      setTonerSaving(p.tonerSave);
      setTonerDensity(p.tonerDensity);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!docName.trim()) {
      setFormError('Document filename is required before submitting to the spooler.');
      return;
    }
    setFormError(null);
    onSubmitNewJob({
      documentName:
        orientation === 'Landscape' && !docName.includes('[Landscape]')
          ? `${docName.trim()} [Landscape]`
          : docName.trim(),
      sourceApp,
      pages: Math.max(1, Math.min(100, pageCount)),
      preset: selectedPreset,
      algorithm: activePresetObj.algorithm,
      paperSize,
      dpi,
      tonerDensity,
      smoothEnable,
      tonerSaving,
    });
  };

  const handleExportDiagnostics = () => {
    const payload = {
      exportedAt: new Date().toISOString(),
      printerModel: 'Canon LaserShot LBP-1120 (0x01A4)',
      usbVidPid: '04A9:262B',
      registers: {
        basicHex: `0x${basicReg.toString(16).toUpperCase().padStart(2, '0')}`,
        auxHex: `0x${auxReg.toString(16).toUpperCase().padStart(2, '0')}`,
        engineHex: `0x${engineReg.toString(16).toUpperCase().padStart(4, '0')}`,
        paperHex: `0x${paperReg.toString(16).toUpperCase().padStart(2, '0')}`,
        shippedPagesCounter,
      },
      faultState: printerFault,
      jobs,
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], {
      type: 'application/json',
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `lbp1120_diagnostics_${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const inspectedJob =
    jobs.find((j) => j.id === selectedJobId) || jobs[0] || null;

  return (
    <div className="space-y-6">
      {/* Top Hardware Status & Telemetry Strip */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
          <div className="flex items-center justify-between text-xs text-slate-500 dark:text-slate-400 mb-1.5">
            <span>USB Transport &amp; Readiness</span>
            <span className="font-mono tabular-nums">04A9:262B</span>
          </div>
          <div className="flex items-center gap-2.5">
            {anyFault ? (
              <AlertTriangle className="w-5 h-5 text-amber-500 shrink-0" />
            ) : (
              <CheckCircle2 className="w-5 h-5 text-emerald-500 shrink-0" />
            )}
            <div>
              <div className="text-base font-semibold text-slate-900 dark:text-slate-100">
                {!printerFault.usbConnected
                  ? 'USB Disconnected'
                  : anyFault
                  ? 'Hardware Attention Required'
                  : activeJob
                  ? `Active: ${activeJob.stage}`
                  : 'Online & Unit Free'}
              </div>
              <div className="text-xs text-slate-500 dark:text-slate-400">
                WinUSB Bulk OUT 0x01 · IN 0x82
              </div>
            </div>
          </div>
        </div>

        <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
          <div className="flex items-center justify-between text-xs text-slate-500 dark:text-slate-400 mb-1.5">
            <span>CAPT 1.0 Hardware Registers</span>
            <span className="font-mono tabular-nums">Opcode 0xA0A0</span>
          </div>
          <div className="grid grid-cols-2 gap-x-3 gap-y-1 text-xs font-mono tabular-nums">
            <div className="text-slate-500 dark:text-slate-400">
              Basic:{' '}
              <span className="font-semibold text-slate-900 dark:text-slate-100">
                0x{basicReg.toString(16).toUpperCase().padStart(2, '0')}
              </span>
            </div>
            <div className="text-slate-500 dark:text-slate-400">
              Engine:{' '}
              <span
                className={`font-semibold ${
                  engineReg !== 0
                    ? 'text-red-600 dark:text-red-400'
                    : 'text-slate-900 dark:text-slate-100'
                }`}
              >
                0x{engineReg.toString(16).toUpperCase().padStart(4, '0')}
              </span>
            </div>
            <div className="text-slate-500 dark:text-slate-400">
              Aux:{' '}
              <span className="font-semibold text-slate-900 dark:text-slate-100">
                0x{auxReg.toString(16).toUpperCase().padStart(2, '0')}
              </span>
            </div>
            <div className="text-slate-500 dark:text-slate-400">
              Paper:{' '}
              <span className="font-semibold text-slate-900 dark:text-slate-100">
                0x{paperReg.toString(16).toUpperCase().padStart(2, '0')}
              </span>
            </div>
          </div>
        </div>

        <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
          <div className="flex items-center justify-between text-xs text-slate-500 dark:text-slate-400 mb-1.5">
            <span>Onboard 512 KB FIFO RAM</span>
            <span className="font-mono tabular-nums">
              {(currentFifoBytes / 1024).toFixed(1)} / 512.0 KB
            </span>
          </div>
          <div className="w-full h-2.5 bg-slate-100 dark:bg-slate-800 rounded overflow-hidden mb-1.5">
            <div
              className={`h-full transition-transform duration-150 origin-left ${
                currentFifoBytes > LBP1120_SAFE_GOVERNOR_BYTES
                  ? 'bg-amber-500'
                  : 'bg-blue-600 dark:bg-blue-500'
              }`}
              style={{ transform: `scaleX(${fifoPercent / 100})` }}
            />
          </div>
          <div className="text-xs text-slate-500 dark:text-slate-400">
            Governor Limit: 480 KB · Utilization: {fifoPercent}%
          </div>
        </div>

        <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
          <div className="flex items-center justify-between text-xs text-slate-500 dark:text-slate-400 mb-1.5">
            <span>Confirmed Physical Output</span>
            <span className="font-mono tabular-nums">Exit Sensor</span>
          </div>
          <div className="text-2xl font-bold font-mono tabular-nums text-slate-900 dark:text-slate-100">
            {shippedPagesCounter} <span className="text-xs font-normal text-slate-500">sheets</span>
          </div>
          <div className="text-xs text-slate-500 dark:text-slate-400">
            Spooler: TCP 9100/9101 · Named Pipes Ready
          </div>
        </div>
      </div>

      {/* Main Two-Column Grid: Left = Print Config & Job Submission, Right = Fault Simulator & Audio Alerts */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-stretch">
        {/* Print Configuration & Job Submission */}
        <form
          onSubmit={handleSubmit}
          className="lg:col-span-7 p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 flex flex-col justify-between space-y-4"
        >
          <div>
            <div className="flex items-center justify-between border-b border-slate-200 dark:border-slate-800 pb-3 mb-4">
              <div className="flex items-center gap-2">
                <Sliders className="w-4 h-4 text-blue-600 dark:text-blue-400" />
                <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">
                  Hardware-Verified Print Configuration &amp; Job Dispatch
                </h2>
              </div>
              <span className="text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
                TargetModel 0x01A4 · 34B Header
              </span>
            </div>

            {formError && (
              <div
                role="alert"
                className="mb-4 p-3 rounded-lg border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950/50 text-xs text-red-700 dark:text-red-300"
              >
                {formError}
              </div>
            )}

            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-4">
              <div className="sm:col-span-2">
                <label
                  htmlFor="doc-name-input"
                  className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1"
                >
                  Document / Spool Payload Name
                </label>
                <input
                  id="doc-name-input"
                  type="text"
                  value={docName}
                  onChange={(e) => setDocName(e.target.value)}
                  placeholder="e.g. Technical_Manual_A4.pdf"
                  className="w-full px-3 py-2 text-sm rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                />
              </div>

              <div>
                <label
                  htmlFor="page-count-input"
                  className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1"
                >
                  Page Count
                </label>
                <input
                  id="page-count-input"
                  type="number"
                  min={1}
                  max={50}
                  value={pageCount}
                  onChange={(e) => setPageCount(Number(e.target.value) || 1)}
                  className="w-full px-3 py-2 text-sm font-mono tabular-nums rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                />
              </div>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-4">
              <div>
                <label
                  htmlFor="preset-select"
                  className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1"
                >
                  Intelligent Processing Preset
                </label>
                <select
                  id="preset-select"
                  value={selectedPreset}
                  onChange={(e) => handlePresetChange(e.target.value as PresetId)}
                  className="w-full px-3 py-2 text-sm rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                >
                  {HALFTONE_PRESETS.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label
                  htmlFor="paper-size-select"
                  className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1"
                >
                  Paper Size (IC_BEGIN_PAGE)
                </label>
                <select
                  id="paper-size-select"
                  value={paperSize}
                  onChange={(e) => setPaperSize(e.target.value as PaperSizeCode)}
                  className="w-full px-3 py-2 text-sm rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                >
                  <option value="A4">A4 (210 × 297 mm · Code 2)</option>
                  <option value="Letter">US Letter (8.5 × 11 in · Code 1)</option>
                  <option value="Legal">US Legal (8.5 × 14 in · Code 3)</option>
                  <option value="Executive">Executive (7.25 × 10.5 in · Code 4)</option>
                  <option value="B5">JIS B5 (182 × 257 mm · Code 5)</option>
                </select>
              </div>

              <div>
                <label
                  htmlFor="source-channel-select"
                  className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1"
                >
                  Spooler Ingestion Channel
                </label>
                <select
                  id="source-channel-select"
                  value={sourceApp}
                  onChange={(e) => setSourceApp(e.target.value)}
                  className="w-full px-3 py-2 text-sm rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                >
                  <option value="WinRT PDF Spooler (TCP 9100)">
                    TCP Port 9100 (Standard Raw)
                  </option>
                  <option value="Named Pipe (\\.\pipe\CanonLBP1120)">
                    Named Pipe (\\.\pipe\CanonLBP1120)
                  </option>
                  <option value="Photo Pipe (\\.\pipe\CanonLBP1120_Photo)">
                    Photo Pipe (\\.\pipe\CanonLBP1120_Photo)
                  </option>
                  <option value="Linux CUPS Filter (rastertocapt1120)">
                    Linux CUPS (rastertocapt1120)
                  </option>
                </select>
              </div>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4 mb-4">
              <div>
                <span className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                  Engine Resolution
                </span>
                <div className="flex p-1 rounded-lg bg-slate-100 dark:bg-slate-800">
                  <button
                    type="button"
                    onClick={() => setDpi(600)}
                    className={`flex-1 py-1.5 px-2 text-xs font-medium rounded-md transition-colors whitespace-nowrap ${
                      dpi === 600
                        ? 'bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 shadow-xs'
                        : 'text-slate-600 dark:text-slate-400'
                    }`}
                  >
                    600 DPI (0x11)
                  </button>
                  <button
                    type="button"
                    onClick={() => setDpi(300)}
                    className={`flex-1 py-1.5 px-2 text-xs font-medium rounded-md transition-colors whitespace-nowrap ${
                      dpi === 300
                        ? 'bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 shadow-xs'
                        : 'text-slate-600 dark:text-slate-400'
                    }`}
                  >
                    300 DPI (0x00)
                  </button>
                </div>
              </div>

              <div>
                <span className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                  Page Orientation
                </span>
                <div className="flex p-1 rounded-lg bg-slate-100 dark:bg-slate-800">
                  <button
                    type="button"
                    onClick={() => setOrientation('Portrait')}
                    className={`flex-1 py-1.5 px-2 text-xs font-medium rounded-md transition-colors whitespace-nowrap ${
                      orientation === 'Portrait'
                        ? 'bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 shadow-xs'
                        : 'text-slate-600 dark:text-slate-400'
                    }`}
                  >
                    Portrait
                  </button>
                  <button
                    type="button"
                    onClick={() => setOrientation('Landscape')}
                    className={`flex-1 py-1.5 px-2 text-xs font-medium rounded-md transition-colors whitespace-nowrap ${
                      orientation === 'Landscape'
                        ? 'bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 shadow-xs'
                        : 'text-slate-600 dark:text-slate-400'
                    }`}
                  >
                    Landscape (90°)
                  </button>
                </div>
              </div>

              <div>
                <label
                  htmlFor="toner-density-slider"
                  className="flex items-center justify-between text-xs font-medium text-slate-700 dark:text-slate-300 mb-1"
                >
                  <span>Laser Toner Density</span>
                  <span className="font-mono tabular-nums">{tonerDensity} / 63</span>
                </label>
                <input
                  id="toner-density-slider"
                  type="range"
                  min={8}
                  max={63}
                  value={tonerDensity}
                  onChange={(e) => setTonerDensity(Number(e.target.value))}
                  className="w-full accent-blue-600 mt-1.5"
                />
              </div>
            </div>

            <div className="flex flex-wrap items-center justify-between gap-4 pt-2 border-t border-slate-100 dark:border-slate-800/80 text-xs text-slate-600 dark:text-slate-400">
              <div className="flex items-center gap-5">
                <label className="inline-flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={smoothEnable}
                    onChange={(e) => setSmoothEnable(e.target.checked)}
                    className="rounded border-slate-300 dark:border-slate-700 accent-blue-600"
                  />
                  <span>AIR Image Refinement (Byte 18 = 0x02)</span>
                </label>
                <label className="inline-flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={tonerSaving}
                    onChange={(e) => setTonerSaving(e.target.checked)}
                    className="rounded border-slate-300 dark:border-slate-700 accent-blue-600"
                  />
                  <span>Economy Toner-Save (Byte 19 = 0x01)</span>
                </label>
              </div>
              <div className="font-mono tabular-nums text-slate-500">
                Canvas: {(paperGeom.imageLineSize || 592) * 8}×{paperGeom.imageLines || 6778} px · Margins: 112L/120T
              </div>
            </div>
          </div>

          <div className="flex items-center justify-between gap-4 pt-3 border-t border-slate-200 dark:border-slate-800">
            <div className="text-xs text-slate-500 dark:text-slate-400">
              Active Halftone: <span className="font-medium text-slate-800 dark:text-slate-200">{ALGORITHM_LABELS[activePresetObj.algorithm]}</span>
            </div>
            <button
              type="submit"
              className="px-4 py-2 text-xs font-semibold text-white bg-blue-600 hover:bg-blue-500 rounded-lg transition-colors flex items-center gap-2 whitespace-nowrap shrink-0 cursor-pointer"
            >
              <Printer className="w-4 h-4" />
              Dispatch Job to Spool Queue
            </button>
          </div>
        </form>

        {/* Right Column: Hardware Fault Injection, Recovery & Canon Audio Status Synthesizer */}
        <div className="lg:col-span-5 p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 flex flex-col justify-between space-y-4">
          <div>
            <div className="flex items-center justify-between border-b border-slate-200 dark:border-slate-800 pb-3 mb-3">
              <div className="flex items-center gap-2">
                <ShieldAlert className="w-4 h-4 text-amber-500" />
                <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">
                  Hardware Fault Simulation &amp; Recovery Lab
                </h2>
              </div>
              <button
                type="button"
                onClick={onRunHardwareRecovery}
                className="px-2.5 py-1 text-xs font-medium rounded-md border border-slate-300 dark:border-slate-700 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300 flex items-center gap-1.5 whitespace-nowrap cursor-pointer"
              >
                <RotateCcw className="w-3.5 h-3.5" />
                Run Full Recovery
              </button>
            </div>

            <p className="text-xs text-slate-500 dark:text-slate-400 mb-3">
              Inject real-world hardware faults into the virtual LBP-1120 endpoint to verify job state machine error handling and recovery sequences.
            </p>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5 mb-4">
              <button
                type="button"
                onClick={() => onToggleFault('paperEmpty')}
                className={`px-3 py-2 rounded-lg border text-left text-xs transition-colors cursor-pointer ${
                  printerFault.paperEmpty
                    ? 'border-amber-500 bg-amber-500/10 text-amber-700 dark:text-amber-300 font-semibold'
                    : 'border-slate-200 dark:border-slate-800 hover:bg-slate-50 dark:hover:bg-slate-800/60 text-slate-700 dark:text-slate-300'
                }`}
              >
                <div className="flex items-center justify-between">
                  <span>Tray Paper Empty</span>
                  <span className="font-mono">0x0200</span>
                </div>
                <div className="text-[11px] text-slate-500 dark:text-slate-400 mt-0.5">
                  {printerFault.paperEmpty ? 'FAULT ACTIVE (No Paper)' : 'Nominal (Paper Loaded)'}
                </div>
              </button>

              <button
                type="button"
                onClick={() => onToggleFault('doorOpen')}
                className={`px-3 py-2 rounded-lg border text-left text-xs transition-colors cursor-pointer ${
                  printerFault.doorOpen
                    ? 'border-red-500 bg-red-500/10 text-red-700 dark:text-red-300 font-semibold'
                    : 'border-slate-200 dark:border-slate-800 hover:bg-slate-50 dark:hover:bg-slate-800/60 text-slate-700 dark:text-slate-300'
                }`}
              >
                <div className="flex items-center justify-between">
                  <span>Top Door / Cover Open</span>
                  <span className="font-mono">0x4000</span>
                </div>
                <div className="text-[11px] text-slate-500 dark:text-slate-400 mt-0.5">
                  {printerFault.doorOpen ? 'FAULT ACTIVE (Door Open)' : 'Nominal (Cover Closed)'}
                </div>
              </button>

              <button
                type="button"
                onClick={() => onToggleFault('paperJam')}
                className={`px-3 py-2 rounded-lg border text-left text-xs transition-colors cursor-pointer ${
                  printerFault.paperJam
                    ? 'border-red-500 bg-red-500/10 text-red-700 dark:text-red-300 font-semibold'
                    : 'border-slate-200 dark:border-slate-800 hover:bg-slate-50 dark:hover:bg-slate-800/60 text-slate-700 dark:text-slate-300'
                }`}
              >
                <div className="flex items-center justify-between">
                  <span>Paper Jam in Fuser</span>
                  <span className="font-mono">0x0100</span>
                </div>
                <div className="text-[11px] text-slate-500 dark:text-slate-400 mt-0.5">
                  {printerFault.paperJam ? 'FAULT ACTIVE (Jam Latched)' : 'Nominal (Path Clear)'}
                </div>
              </button>

              <button
                type="button"
                onClick={() => onToggleFault('noCartridge')}
                className={`px-3 py-2 rounded-lg border text-left text-xs transition-colors cursor-pointer ${
                  printerFault.noCartridge
                    ? 'border-red-500 bg-red-500/10 text-red-700 dark:text-red-300 font-semibold'
                    : 'border-slate-200 dark:border-slate-800 hover:bg-slate-50 dark:hover:bg-slate-800/60 text-slate-700 dark:text-slate-300'
                }`}
              >
                <div className="flex items-center justify-between">
                  <span>EP-22 Toner Cartridge</span>
                  <span className="font-mono">0x2000</span>
                </div>
                <div className="text-[11px] text-slate-500 dark:text-slate-400 mt-0.5">
                  {printerFault.noCartridge ? 'FAULT ACTIVE (Missing)' : 'Nominal (Seated)'}
                </div>
              </button>
            </div>
          </div>

          {/* Authentic Canon Audio Alert Synthesizer (Extracted from CAP3ZWAV.BIN) */}
          <div className="pt-3 border-t border-slate-200 dark:border-slate-800">
            <div className="flex items-center justify-between mb-2">
              <span className="text-xs font-semibold text-slate-800 dark:text-slate-200 flex items-center gap-1.5">
                <Volume2 className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" />
                Authentic Canon Voice Alerts (CAP3ZWAV.BIN)
              </span>
              {activeSoundFile && (
                <span className="text-xs font-mono text-blue-600 dark:text-blue-400">
                  Playing: {activeSoundFile}
                </span>
              )}
            </div>
            <div className="flex flex-wrap gap-1.5">
              {(availableSounds.length > 0
                ? availableSounds.slice(0, 8)
                : [
                    { filename: 'Cap3strt.wav', label: 'PrintStart', sizeBytes: 21957 },
                    { filename: 'Cap3done.wav', label: 'PrintComplete', sizeBytes: 22621 },
                    { filename: 'Cap3addp.wav', label: 'AddPaper', sizeBytes: 27268 },
                    { filename: 'Cap3covr.wav', label: 'CoverOpen', sizeBytes: 27268 },
                    { filename: 'Cap3jam.wav', label: 'PaperJam', sizeBytes: 16644 },
                    { filename: 'Cap3cart.wav', label: 'NoCartridge', sizeBytes: 34572 },
                  ]
              ).map((snd) => (
                <button
                  key={snd.filename}
                  type="button"
                  onClick={() => onPlayCanonSound(snd.filename)}
                  className="px-2.5 py-1 text-xs font-mono rounded border border-slate-200 dark:border-slate-800 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300 transition-colors cursor-pointer"
                >
                  {snd.filename.replace('.wav', '')}
                </button>
              ))}
            </div>
          </div>
        </div>
      </div>

      {/* Deterministic 7-Stage Print Job Queue & Lifecycle Trace */}
      <div className="p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
        <div className="flex flex-wrap items-center justify-between gap-4 border-b border-slate-200 dark:border-slate-800 pb-4 mb-4">
          <div>
            <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100 flex items-center gap-2">
              <Activity className="w-4 h-4 text-blue-600 dark:text-blue-400" />
              Traceable Print Job State Machine &amp; Queue Management
            </h2>
            <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
              Lifecycle: Submitted → Accepted → Rendering → Processing → Transmitting (USB 0x01) → Printing (Mechanical Feed) → Completed (Exit Sensor Confirmed)
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={ onClearFinishedJobs }
              className="px-3 py-1.5 text-xs font-medium rounded-lg border border-slate-300 dark:border-slate-700 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300 flex items-center gap-1.5 whitespace-nowrap cursor-pointer"
            >
              <Trash2 className="w-3.5 h-3.5" />
              Clear Finished / Cancelled
            </button>
            <button
              type="button"
              onClick={handleExportDiagnostics}
              className="px-3 py-1.5 text-xs font-medium rounded-lg border border-slate-300 dark:border-slate-700 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300 flex items-center gap-1.5 whitespace-nowrap cursor-pointer"
            >
              <Download className="w-3.5 h-3.5" />
              Export Diagnostics JSON
            </button>
          </div>
        </div>

        {jobs.length === 0 ? (
          <div className="py-10 text-center text-sm text-slate-500 dark:text-slate-400">
            No print jobs currently in queue. Use the dispatch form above to submit a document.
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left border-collapse">
              <thead>
                <tr className="border-b border-slate-200 dark:border-slate-800 text-xs font-medium text-slate-500 dark:text-slate-400">
                  <th className="py-2.5 pr-4">Job ID &amp; Document</th>
                  <th className="py-2.5 px-3">Mode &amp; Halftone</th>
                  <th className="py-2.5 px-3">Lifecycle Stage (7-Step Pipeline)</th>
                  <th className="py-2.5 px-3 text-right">SCoA Stream</th>
                  <th className="py-2.5 px-3 text-right">Exit Counter</th>
                  <th className="py-2.5 pl-3 text-right">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-sm">
                {jobs.map((job) => {
                  const isSelected = inspectedJob?.id === job.id;
                  return (
                    <tr
                      key={job.id}
                      onClick={() => setSelectedJobId(job.id)}
                      className={`transition-colors cursor-pointer ${
                        isSelected
                          ? 'bg-blue-50/50 dark:bg-blue-950/20'
                          : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                      }`}
                    >
                      <td className="py-3 pr-4">
                        <div className="font-medium text-slate-900 dark:text-slate-100 flex items-center gap-2">
                          <FileText className="w-4 h-4 text-slate-400 shrink-0" />
                          <span>{job.documentName}</span>
                        </div>
                        <div className="text-xs text-slate-500 dark:text-slate-400 font-mono tabular-nums mt-0.5">
                          {job.id} · {job.pages} pg · {job.sourceApp}
                        </div>
                      </td>

                      <td className="py-3 px-3 text-xs text-slate-600 dark:text-slate-300">
                        <div>{ALGORITHM_LABELS[job.algorithm]}</div>
                        <div className="text-slate-400">Preset: {job.preset}</div>
                      </td>

                      <td className="py-3 px-3">
                        <div className="flex items-center gap-1.5 mb-1">
                          {job.stage === 'Completed' ? (
                            <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
                          ) : job.stage === 'Failed' || job.stage === 'Cancelled' ? (
                            <XCircle className="w-4 h-4 text-red-500 shrink-0" />
                          ) : (
                            <RefreshCw className="w-4 h-4 text-blue-500 shrink-0" />
                          )}
                          <span
                            className={`text-xs font-semibold ${
                              job.stage === 'Completed'
                                ? 'text-emerald-600 dark:text-emerald-400'
                                : job.stage === 'Failed' || job.stage === 'Cancelled'
                                ? 'text-red-600 dark:text-red-400'
                                : 'text-blue-600 dark:text-blue-400'
                            }`}
                          >
                            {job.stage}
                          </span>
                          <span className="text-xs text-slate-400 font-mono tabular-nums">
                            ·{' '}
                            {job.stage === 'Transmitting'
                              ? 'USB Transfer Only (Unconfirmed Physical)'
                              : job.stage === 'Printing'
                              ? 'Fuser & Feed Active'
                              : job.stage === 'Completed'
                              ? 'Hardware Exit Sensor Verified'
                              : `Step ${job.stageIndex + 1}/7`}
                          </span>
                        </div>
                        {/* 7-Segment Pipeline Progress Bar */}
                        <div className="grid grid-cols-7 gap-1 w-56">
                          {JOB_STAGE_SEQUENCE.map((st, idx) => {
                            const passed = job.stageIndex >= idx && job.stage !== 'Cancelled';
                            const failedHere = job.stage === 'Failed' && job.stageIndex === idx;
                            return (
                              <div
                                key={st}
                                title={st}
                                className={`h-1.5 rounded-xs ${
                                  failedHere
                                    ? 'bg-red-500'
                                    : passed
                                    ? job.stage === 'Completed'
                                      ? 'bg-emerald-500'
                                      : 'bg-blue-600 dark:bg-blue-500'
                                    : 'bg-slate-200 dark:bg-slate-800'
                                }`}
                              />
                            );
                          })}
                        </div>
                        {job.errorReason && (
                          <div className="text-xs text-red-600 dark:text-red-400 mt-1">
                            {job.errorReason}
                          </div>
                        )}
                      </td>

                      <td className="py-3 px-3 text-right font-mono tabular-nums text-xs text-slate-700 dark:text-slate-300">
                        <div>{(job.scoaBytes / 1024).toFixed(1)} KB</div>
                        <div className="text-slate-400">
                          {job.usbPacketsSent}/{job.usbPacketsTotal} pkts
                        </div>
                      </td>

                      <td className="py-3 px-3 text-right font-mono tabular-nums text-xs text-slate-700 dark:text-slate-300">
                        <div>
                          {job.baselineShippedCounter} → {job.currentShippedCounter}
                        </div>
                        <div className="text-slate-400">
                          {job.physicalDeliveryConfirmed ? 'Delivered' : 'Pending'}
                        </div>
                      </td>

                      <td
                        className="py-3 pl-3 text-right"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <div className="inline-flex items-center gap-1.5">
                          {job.stage !== 'Completed' &&
                            job.stage !== 'Failed' &&
                            job.stage !== 'Cancelled' && (
                              <>
                                <button
                                  type="button"
                                  onClick={() => onStepJob(job.id)}
                                  className="px-2.5 py-1 text-xs font-medium rounded border border-blue-600/40 bg-blue-600/10 text-blue-600 dark:text-blue-400 hover:bg-blue-600/20 whitespace-nowrap cursor-pointer"
                                >
                                  Step Stage
                                </button>
                                <button
                                  type="button"
                                  onClick={() => onAutoCompleteJob(job.id)}
                                  aria-label={`Run ${job.documentName} to completion`}
                                  className="px-2.5 py-1 text-xs font-medium rounded bg-blue-600 text-white hover:bg-blue-500 flex items-center gap-1 whitespace-nowrap cursor-pointer"
                                >
                                  <Play className="w-3 h-3" />
                                  Run
                                </button>
                                <button
                                  type="button"
                                  onClick={() => onCancelJob(job.id)}
                                  className="px-2 py-1 text-xs rounded border border-slate-300 dark:border-slate-700 text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 whitespace-nowrap cursor-pointer"
                                >
                                  Cancel
                                </button>
                              </>
                            )}
                          {(job.stage === 'Failed' || job.stage === 'Cancelled') && (
                            <button
                              type="button"
                              onClick={() => onRetryJob(job.id)}
                              className="px-2.5 py-1 text-xs font-medium rounded bg-amber-600 text-white hover:bg-amber-500 flex items-center gap-1 whitespace-nowrap cursor-pointer"
                            >
                              <RotateCcw className="w-3 h-3" />
                              Retry Job
                            </button>
                          )}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}

        {/* Selected Job Audit Log */}
        {inspectedJob && (
          <div className="mt-4 pt-4 border-t border-slate-200 dark:border-slate-800">
            <div className="text-xs font-semibold text-slate-700 dark:text-slate-300 mb-2">
              State Transition Audit Trail — {inspectedJob.id} ({inspectedJob.documentName})
            </div>
            <div className="space-y-1.5 max-h-36 overflow-y-auto font-mono text-xs">
              {inspectedJob.transitionLog.map((entry, idx) => (
                <div
                  key={`${entry.stage}-${idx}`}
                  className="flex items-start gap-3 text-slate-600 dark:text-slate-400"
                >
                  <span className="tabular-nums text-slate-400 shrink-0">
                    [{entry.timestamp}]
                  </span>
                  <span className="font-semibold text-slate-800 dark:text-slate-200 w-28 shrink-0">
                    {entry.stage}
                  </span>
                  <span>{entry.detail}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
