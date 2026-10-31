import test from 'node:test';
import assert from 'node:assert/strict';
import {
  DEFAULT_A4_CONFIG,
  OPCODE_IC_BEGIN_PAGE,
  OPCODE_PCR_GO_ONLINE,
  TARGET_MODEL_LBP1120,
  ScoaDecoder,
  ScoaEncoder,
  advanceJobState,
  buildBeginPagePayload,
  buildCaptPacket,
  buildGoOnlinePacket,
  computeOtsuThreshold,
  decodeCaptStreamToPbm,
  encodePbmToCaptStream,
  generateReferenceTestImage,
  parseBeginPagePayload,
  parseCaptPacket,
  runHalftoneAlgorithm,
  type HalftoneAlgorithmId,
  type PrintJobItem,
} from './captEngine';

test('CAPT 1.0 packet header framing and IC_BEGIN_PAGE 34-byte roundtrip', () => {
  const payload = buildBeginPagePayload(DEFAULT_A4_CONFIG);
  assert.equal(payload.length, 34);

  const pkt = buildCaptPacket(OPCODE_IC_BEGIN_PAGE, payload);
  assert.equal(pkt.length, 38);

  const parsedPkt = parseCaptPacket(pkt, 0);
  assert.equal(parsedPkt.opcode, OPCODE_IC_BEGIN_PAGE);
  assert.equal(parsedPkt.totalSize, 38);

  const parsedCfg = parseBeginPagePayload(parsedPkt.payload);
  assert.equal(parsedCfg.targetModel, TARGET_MODEL_LBP1120);
  assert.equal(parsedCfg.imageLineSize, 592);
  assert.equal(parsedCfg.imageLines, 6778);
  assert.equal(parsedCfg.paperWidth, 4960);
  assert.equal(parsedCfg.paperHeight, 7015);
  assert.equal(parsedCfg.resolution, 0x11);
});

test('PCR_GO_ONLINE packet embeds magic 0xADEADBEE and page index', () => {
  const pkt = buildGoOnlinePacket(3);
  assert.equal(pkt.length, 12);
  const parsed = parseCaptPacket(pkt);
  assert.equal(parsed.opcode, OPCODE_PCR_GO_ONLINE);
  // 0xADEADBEE in little-endian: EE DB EA AD
  assert.deepEqual(Array.from(parsed.payload.slice(0, 4)), [0xee, 0xdb, 0xea, 0xad]);
  assert.equal(parsed.payload[4], 3);
  assert.equal(parsed.payload[5], 0);
});

test('SCoA 2D Delta + RLE encoder and decoder bit-exact roundtrip across patterns', () => {
  const lineSize = 64;
  const lines = 24;
  const original = new Uint8Array(lineSize * lines);

  // Fill with diverse patterns: all-white, all-black, REPEAT_X_BYTE (0x43), alternating stripes, pseudo-random
  for (let y = 0; y < lines; y++) {
    const row = original.subarray(y * lineSize, (y + 1) * lineSize);
    if (y < 4) {
      row.fill(0x00);
    } else if (y < 8) {
      row.fill(0xff);
    } else if (y < 12) {
      row.fill(0x43); // REPEAT_X_BYTE
    } else if (y < 16) {
      row.fill(y % 2 === 0 ? 0xaa : 0x55);
    } else {
      for (let x = 0; x < lineSize; x++) {
        row[x] = (x * 37 + y * 13) & 0xff;
      }
    }
  }

  const encoder = new ScoaEncoder(lineSize);
  for (let y = 0; y < lines; y++) {
    encoder.encodeLine(original.subarray(y * lineSize, (y + 1) * lineSize));
  }
  const compressed = encoder.finish();
  assert.equal(compressed.length % 2, 0, 'SCoA stream must be 16-bit word aligned');
  assert.equal(compressed[compressed.length - 1], 0x42, 'Final byte must be OP_EOP (0x42)');

  const decoder = new ScoaDecoder(lineSize);
  const recovered = decoder.decodeStream(compressed);
  assert.equal(decoder.eopReached, true);
  assert.deepEqual(recovered, original);
});

test('Full CAPT page stream encode and decode roundtrip', () => {
  const lineSize = 48;
  const lines = 16;
  const pbm = new Uint8Array(lineSize * lines);
  for (let i = 0; i < pbm.length; i++) {
    pbm[i] = i % 7 === 0 ? 0xf0 : 0x00;
  }

  const { captStream, packetCount } = encodePbmToCaptStream(pbm, lineSize, lines, DEFAULT_A4_CONFIG, 128);
  assert.ok(packetCount >= 4);

  const decoded = decodeCaptStreamToPbm(captStream);
  assert.equal(decoded.config.imageLineSize, lineSize);
  assert.equal(decoded.config.imageLines, lines);
  assert.deepEqual(decoded.pbmData, pbm);
});

test('All 9 monochrome halftoning algorithms produce valid 1-bit rasters and SCoA streams', () => {
  const width = 128;
  const height = 96;
  const refImg = generateReferenceTestImage('mixed_technical', width, height);

  const algorithms: HalftoneAlgorithmId[] = [
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

  for (const alg of algorithms) {
    const res = runHalftoneAlgorithm(refImg, width, height, alg, {
      gamma: 1.05,
      contrast: 8,
      sharpen: 0.3,
    });
    assert.equal(res.totalPixels, width * height);
    assert.ok(res.blackPixels > 0 && res.blackPixels < width * height, `${alg} produced invalid black pixel count`);
    assert.ok(res.scoaCompressedBytes > 0);
    assert.equal(res.pbmPacked.length, Math.ceil(width / 8) * height);
  }
});

test('Otsu threshold separates bimodal dark text and light background', () => {
  const sample = new Uint8Array(200);
  sample.fill(240, 0, 150);
  sample.fill(20, 150, 200);
  const t = computeOtsuThreshold(sample);
  assert.ok(t >= 20 && t < 240);
});

test('Print job state machine enforces 7-stage progression and distinguishes Transmitting from Printing', () => {
  let job: PrintJobItem = {
    id: 'JOB-101',
    documentName: 'Engineering_Spec.pdf',
    sourceApp: 'Microsoft Word',
    pages: 2,
    preset: 'mixed_document',
    algorithm: 'normal_clustered_8x8',
    stage: 'Submitted',
    stageIndex: 0,
    submittedAt: '07:00:00',
    scoaBytes: 42800,
    usbPacketsSent: 0,
    usbPacketsTotal: 11,
    baselineShippedCounter: 10,
    currentShippedCounter: 10,
    physicalDeliveryConfirmed: false,
    transitionLog: [],
  };

  const healthyPrinter = {
    doorOpen: false,
    paperEmpty: false,
    paperJam: false,
    noCartridge: false,
    usbConnected: true,
  };

  let shipped = 10;
  const expectedStages = ['Accepted', 'Rendering', 'Processing', 'Transmitting', 'Printing', 'Completed'];

  for (const expected of expectedStages) {
    const step = advanceJobState(job, healthyPrinter, shipped);
    job = step.updatedJob;
    shipped = step.newShippedCounter;
    assert.equal(job.stage, expected);

    if (expected === 'Transmitting') {
      // USB data transfer finished, but physical printing is NOT yet completed!
      assert.equal(job.physicalDeliveryConfirmed, false);
      assert.equal(shipped, 10);
    }
    if (expected === 'Completed') {
      assert.equal(job.physicalDeliveryConfirmed, true);
      assert.equal(shipped, 12);
    }
  }
});
