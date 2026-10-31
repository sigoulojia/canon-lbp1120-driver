import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  CheckCircle2,
  Download,
  Eye,
  Layers,
  Sliders,
  Sparkles,
  AlertTriangle,
} from 'lucide-react';
import {
  ALGORITHM_LABELS,
  HALFTONE_PRESETS,
  generateReferenceTestImage,
  runHalftoneAlgorithm,
  type HalftoneAlgorithmId,
  type HalftoneBenchmarkMetrics,
  type PresetId,
  type ReferencePlateType,
} from '../core/captEngine';

const ALL_ALGORITHMS: HalftoneAlgorithmId[] = [
  'normal_clustered_8x8',
  'canon_cap3ht_16x16',
  'floyd_steinberg',
  'jarvis_judice_ninke',
  'stucki',
  'atkinson',
  'ordered_bayer_8x8',
  'otsu_global',
  'sauvola_adaptive',
];

export const HalftoneLabTab: React.FC = () => {
  const [plate, setPlate] = useState<ReferencePlateType>('mixed_technical');
  const [selectedAlgorithm, setSelectedAlgorithm] =
    useState<HalftoneAlgorithmId>('canon_cap3ht_16x16');
  const [activePreset, setActivePreset] = useState<PresetId>('graphics');
  const [gamma, setGamma] = useState(1.05);
  const [contrast, setContrast] = useState(8);
  const [sharpen, setSharpen] = useState(0.35);
  const [tonerSave, setTonerSave] = useState(false);

  const srcCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const outCanvasRef = useRef<HTMLCanvasElement | null>(null);

  const width = 320;
  const height = 220;

  const sourceGray = useMemo(
    () => generateReferenceTestImage(plate, width, height),
    [plate]
  );

  // Run full 9-algorithm benchmark on the active reference image and pre-processing settings
  const benchmarkResults: HalftoneBenchmarkMetrics[] = useMemo(() => {
    return ALL_ALGORITHMS.map((alg) =>
      runHalftoneAlgorithm(sourceGray, width, height, alg, {
        gamma,
        contrast,
        sharpen,
        tonerSave,
      })
    );
  }, [sourceGray, gamma, contrast, sharpen, tonerSave]);

  const activeResult = useMemo(() => {
    return (
      benchmarkResults.find((r) => r.algorithm === selectedAlgorithm) ||
      benchmarkResults[0]
    );
  }, [benchmarkResults, selectedAlgorithm]);

  // Paint source grayscale canvas
  useEffect(() => {
    const canvas = srcCanvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    const imgData = ctx.createImageData(width, height);
    for (let i = 0; i < sourceGray.length; i++) {
      const v = sourceGray[i];
      const p = i * 4;
      imgData.data[p] = v;
      imgData.data[p + 1] = v;
      imgData.data[p + 2] = v;
      imgData.data[p + 3] = 255;
    }
    ctx.putImageData(imgData, 0, 0);
  }, [sourceGray]);

  // Paint 1-bit halftoned output canvas
  useEffect(() => {
    const canvas = outCanvasRef.current;
    if (!canvas || !activeResult) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    const imgData = ctx.createImageData(width, height);
    const mono = activeResult.monoPixels;
    for (let i = 0; i < mono.length; i++) {
      const v = mono[i];
      const p = i * 4;
      imgData.data[p] = v;
      imgData.data[p + 1] = v;
      imgData.data[p + 2] = v;
      imgData.data[p + 3] = 255;
    }
    ctx.putImageData(imgData, 0, 0);
  }, [activeResult]);

  const applyPreset = (presetId: PresetId) => {
    setActivePreset(presetId);
    const p = HALFTONE_PRESETS.find((item) => item.id === presetId);
    if (p) {
      setSelectedAlgorithm(p.algorithm);
      setGamma(p.gamma);
      setContrast(p.contrast);
      setSharpen(p.sharpen);
      setTonerSave(p.tonerSave);
    }
  };

  const handleDownloadPbm = () => {
    if (!activeResult) return;
    const header = new TextEncoder().encode(
      `P4\n# Canon LBP-1120 CAPT Halftone (${activeResult.algorithm})\n${width} ${height}\n`
    );
    const combined = new Uint8Array(header.length + activeResult.pbmPacked.length);
    combined.set(header, 0);
    combined.set(activeResult.pbmPacked, header.length);
    const blob = new Blob([combined], { type: 'image/x-portable-bitmap' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `lbp1120_${plate}_${activeResult.algorithm}.pbm`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="space-y-6">
      {/* Intelligent Processing Presets Bar */}
      <div className="p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
        <div className="flex flex-wrap items-center justify-between gap-4 mb-3">
          <div>
            <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100 flex items-center gap-2">
              <Sparkles className="w-4 h-4 text-blue-600 dark:text-blue-400" />
              Intelligent Monochrome Processing Presets &amp; Calibration Controls
            </h2>
            <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
              Select a document-tuned preset or fine-tune gamma, contrast, unsharp mask, and halftoning kernel below.
            </p>
          </div>
          <button
            type="button"
            onClick={handleDownloadPbm}
            className="px-3 py-1.5 text-xs font-medium rounded-lg border border-slate-300 dark:border-slate-700 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300 flex items-center gap-1.5 whitespace-nowrap cursor-pointer"
          >
            <Download className="w-3.5 h-3.5" />
            Export 1-Bit Raster (.PBM)
          </button>
        </div>

        <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-2.5 mb-5">
          {HALFTONE_PRESETS.map((p) => {
            const isSelected = activePreset === p.id;
            return (
              <button
                key={p.id}
                type="button"
                onClick={() => applyPreset(p.id)}
                className={`p-3 rounded-lg border text-left transition-colors cursor-pointer ${
                  isSelected
                    ? 'border-blue-600 bg-blue-50/70 dark:bg-blue-950/40 text-slate-900 dark:text-slate-100'
                    : 'border-slate-200 dark:border-slate-800 hover:bg-slate-50 dark:hover:bg-slate-800/50 text-slate-700 dark:text-slate-300'
                }`}
              >
                <div className="text-xs font-semibold">{p.name}</div>
                <div className="text-[11px] text-slate-500 dark:text-slate-400 mt-1 line-clamp-2">
                  {p.description}
                </div>
              </button>
            );
          })}
        </div>

        {/* Sliders & Plate Selector */}
        <div className="grid grid-cols-1 md:grid-cols-5 gap-4 pt-4 border-t border-slate-200 dark:border-slate-800">
          <div>
            <label
              htmlFor="plate-select"
              className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1"
            >
              Reference Test Plate
            </label>
            <select
              id="plate-select"
              value={plate}
              onChange={(e) => setPlate(e.target.value as ReferencePlateType)}
              className="w-full px-3 py-1.5 text-xs rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
            >
              <option value="mixed_technical">Mixed Text, Sphere &amp; Ramp</option>
              <option value="photographic_portrait">Continuous-Tone Studio Spheres</option>
              <option value="degraded_invoice">Uneven Shadowed Document Scan</option>
              <option value="micro_typography">600 DPI Reticle &amp; Micro-Lines</option>
            </select>
          </div>

          <div>
            <label
              htmlFor="alg-select"
              className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1"
            >
              Active Halftone Kernel
            </label>
            <select
              id="alg-select"
              value={selectedAlgorithm}
              onChange={(e) =>
                setSelectedAlgorithm(e.target.value as HalftoneAlgorithmId)
              }
              className="w-full px-3 py-1.5 text-xs rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
            >
              {ALL_ALGORITHMS.map((alg) => (
                <option key={alg} value={alg}>
                  {ALGORITHM_LABELS[alg]}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label
              htmlFor="gamma-slider"
              className="flex justify-between text-xs font-medium text-slate-700 dark:text-slate-300 mb-1"
            >
              <span>Gamma Calibration</span>
              <span className="font-mono tabular-nums">{gamma.toFixed(2)}</span>
            </label>
            <input
              id="gamma-slider"
              type="range"
              min={0.5}
              max={2.2}
              step={0.05}
              value={gamma}
              onChange={(e) => setGamma(Number(e.target.value))}
              className="w-full accent-blue-600"
            />
          </div>

          <div>
            <label
              htmlFor="contrast-slider"
              className="flex justify-between text-xs font-medium text-slate-700 dark:text-slate-300 mb-1"
            >
              <span>Contrast S-Curve</span>
              <span className="font-mono tabular-nums">
                {contrast >= 0 ? `+${contrast}` : contrast}
              </span>
            </label>
            <input
              id="contrast-slider"
              type="range"
              min={-40}
              max={50}
              step={2}
              value={contrast}
              onChange={(e) => setContrast(Number(e.target.value))}
              className="w-full accent-blue-600"
            />
          </div>

          <div>
            <label
              htmlFor="sharpen-slider"
              className="flex justify-between text-xs font-medium text-slate-700 dark:text-slate-300 mb-1"
            >
              <span>3×3 Unsharp Mask</span>
              <span className="font-mono tabular-nums">{sharpen.toFixed(2)}</span>
            </label>
            <input
              id="sharpen-slider"
              type="range"
              min={0}
              max={1.8}
              step={0.05}
              value={sharpen}
              onChange={(e) => setSharpen(Number(e.target.value))}
              className="w-full accent-blue-600"
            />
          </div>
        </div>
      </div>

      {/* Side-by-Side Visual Comparison Viewport */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm font-semibold text-slate-900 dark:text-slate-100 flex items-center gap-2">
              <Eye className="w-4 h-4 text-slate-500" />
              Continuous-Tone 8-Bit Input Source
            </span>
            <span className="text-xs font-mono tabular-nums text-slate-500">
              {width}×{height} px · 256 Gray Levels
            </span>
          </div>
          <div className="rounded-lg overflow-hidden border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-950 flex items-center justify-center p-3">
            <canvas
              ref={srcCanvasRef}
              width={width}
              height={height}
              data-testid="halftone-source-canvas"
              className="w-full max-w-[480px] h-auto rounded border border-slate-300 dark:border-slate-700"
              style={{ imageRendering: 'pixelated' }}
            />
          </div>
        </div>

        <div className="p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
          <div className="flex items-center justify-between gap-2 mb-3">
            <span className="text-sm font-semibold text-slate-900 dark:text-slate-100 flex items-center gap-2 truncate">
              <Sliders className="w-4 h-4 text-blue-600 dark:text-blue-400 shrink-0" />
              <span className="truncate">1-Bit Laser Output ({activeResult.algorithmName})</span>
            </span>
            <span className="text-xs font-mono tabular-nums text-slate-500 whitespace-nowrap shrink-0">
              Coverage: {activeResult.coveragePercent}% · SCoA: {(activeResult.scoaCompressedBytes / 1024).toFixed(1)} KB
            </span>
          </div>
          <div className="rounded-lg overflow-hidden border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-950 flex items-center justify-center p-3">
            <canvas
              ref={outCanvasRef}
              width={width}
              height={height}
              data-testid="halftone-output-canvas"
              className="w-full max-w-[480px] h-auto rounded border border-slate-300 dark:border-slate-700"
              style={{ imageRendering: 'pixelated' }}
            />
          </div>
        </div>
      </div>

      {/* 9-Algorithm Comparative Benchmark Table */}
      <div className="p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
        <div className="flex flex-wrap items-center justify-between gap-4 border-b border-slate-200 dark:border-slate-800 pb-3 mb-4">
          <div>
            <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100 flex items-center gap-2">
              <Layers className="w-4 h-4 text-blue-600 dark:text-blue-400" />
              9-Algorithm Monochrome Halftoning &amp; SCoA Compression Benchmark
            </h2>
            <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
              Click any algorithm row to inspect its 1-bit raster above. Compares execution speed, black toner coverage, SCoA compression ratio, and 512 KB hardware FIFO safety.
            </p>
          </div>
          <label className="inline-flex items-center gap-2 text-xs text-slate-700 dark:text-slate-300 cursor-pointer">
            <input
              type="checkbox"
              checked={tonerSave}
              onChange={(e) => setTonerSave(e.target.checked)}
              className="rounded accent-blue-600"
            />
            <span>Apply Hardware Economy Toner-Save Gate</span>
          </label>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="border-b border-slate-200 dark:border-slate-800 text-xs font-medium text-slate-500 dark:text-slate-400">
                <th className="py-2.5 pr-4">Halftoning / Thresholding Algorithm</th>
                <th className="py-2.5 px-3 text-right">Compute Time</th>
                <th className="py-2.5 px-3 text-right">Black Coverage</th>
                <th className="py-2.5 px-3 text-right">A4 SCoA Stream</th>
                <th className="py-2.5 px-3 text-right">SCoA Ratio</th>
                <th className="py-2.5 px-3 text-right">Text / Edge</th>
                <th className="py-2.5 px-3 text-right">Gradients</th>
                <th className="py-2.5 pl-3 text-right">512 KB FIFO Status</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-sm">
              {benchmarkResults.map((row) => {
                const isSelected = row.algorithm === selectedAlgorithm;
                return (
                  <tr
                    key={row.algorithm}
                    onClick={() => setSelectedAlgorithm(row.algorithm)}
                    className={`transition-colors cursor-pointer ${
                      isSelected
                        ? 'bg-blue-50/60 dark:bg-blue-950/30 font-medium'
                        : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                    }`}
                  >
                    <td className="py-2.5 pr-4 text-slate-900 dark:text-slate-100">
                      {row.algorithmName}
                    </td>
                    <td className="py-2.5 px-3 text-right font-mono tabular-nums text-xs text-slate-700 dark:text-slate-300">
                      {row.processingMs.toFixed(2)} ms
                    </td>
                    <td className="py-2.5 px-3 text-right font-mono tabular-nums text-xs text-slate-700 dark:text-slate-300">
                      {row.coveragePercent.toFixed(2)}%
                    </td>
                    <td className="py-2.5 px-3 text-right font-mono tabular-nums text-xs text-slate-700 dark:text-slate-300">
                      {(row.scoaCompressedBytes / 1024).toFixed(1)} KB
                    </td>
                    <td className="py-2.5 px-3 text-right font-mono tabular-nums text-xs text-slate-700 dark:text-slate-300">
                      {row.scoaRatioPercent.toFixed(1)}%
                    </td>
                    <td className="py-2.5 px-3 text-right font-mono tabular-nums text-xs text-slate-700 dark:text-slate-300">
                      {row.edgePreservationScore.toFixed(1)}
                    </td>
                    <td className="py-2.5 px-3 text-right font-mono tabular-nums text-xs text-slate-700 dark:text-slate-300">
                      {row.gradientSmoothnessScore.toFixed(1)}
                    </td>
                    <td className="py-2.5 pl-3 text-right text-xs">
                      {row.fifoSafe ? (
                        <span className="inline-flex items-center gap-1 text-emerald-600 dark:text-emerald-400 font-mono tabular-nums">
                          <CheckCircle2 className="w-3.5 h-3.5" />
                          Safe ({row.fifoUtilizationPercent}%)
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 text-amber-600 dark:text-amber-400 font-mono tabular-nums">
                          <AlertTriangle className="w-3.5 h-3.5" />
                          Governor Fallback ({row.fifoUtilizationPercent}%)
                        </span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
