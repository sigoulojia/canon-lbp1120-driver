import express from 'express';
import fs from 'fs';
import path from 'path';
import crypto from 'crypto';
import { fileURLToPath } from 'url';
import { createServer as createViteServer } from 'vite';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function parsePeExports(filePath: string): string[] {
  try {
    const buf = fs.readFileSync(filePath);
    if (buf.length < 0x40 || buf[0] !== 0x4d || buf[1] !== 0x5a) return [];
    const peOffset = buf.readUInt32LE(0x3c);
    if (peOffset + 128 > buf.length) return [];
    const exportRva = buf.readUInt32LE(peOffset + 120);
    if (exportRva === 0) return [];
    const numSections = buf.readUInt16LE(peOffset + 6);
    const optHeaderSize = buf.readUInt16LE(peOffset + 20);
    const sectionTableOffset = peOffset + 24 + optHeaderSize;
    const sections: { vAddr: number; vSize: number; rOffset: number; rSize: number }[] = [];
    for (let i = 0; i < numSections; i++) {
      const off = sectionTableOffset + i * 40;
      if (off + 40 > buf.length) break;
      sections.push({
        vSize: buf.readUInt32LE(off + 8),
        vAddr: buf.readUInt32LE(off + 12),
        rSize: buf.readUInt32LE(off + 16),
        rOffset: buf.readUInt32LE(off + 20),
      });
    }
    const rvaToOff = (rva: number) => {
      for (const s of sections) {
        const span = Math.max(s.vSize, s.rSize);
        if (rva >= s.vAddr && rva < s.vAddr + span) {
          return s.rOffset + (rva - s.vAddr);
        }
      }
      return 0;
    };
    const expOff = rvaToOff(exportRva);
    if (!expOff || expOff + 40 > buf.length) return [];
    const numNames = buf.readUInt32LE(expOff + 24);
    const namesRva = buf.readUInt32LE(expOff + 32);
    const namesOff = rvaToOff(namesRva);
    if (!namesOff) return [];
    const names: string[] = [];
    for (let i = 0; i < Math.min(numNames, 32); i++) {
      const nameRva = buf.readUInt32LE(namesOff + i * 4);
      let p = rvaToOff(nameRva);
      if (!p || p >= buf.length) continue;
      let s = '';
      while (p < buf.length && buf[p] !== 0) {
        s += String.fromCharCode(buf[p++]);
      }
      if (s) names.push(s);
    }
    return names;
  } catch {
    return [];
  }
}

function parseCap3htBinary(filePath: string) {
  try {
    const ht = fs.readFileSync(filePath);
    const numTables = ht.readUInt32LE(0);
    const tables = [];
    for (let i = 0; i < numTables; i++) {
      const offset = ht.readUInt32LE(4 + i * 8);
      const length = ht.readUInt32LE(4 + i * 8 + 4);
      tables.push({
        index: i,
        offsetHex: `0x${offset.toString(16).toUpperCase().padStart(5, '0')}`,
        offset,
        length,
        headerHex: ht.subarray(offset, offset + 12).toString('hex').toUpperCase(),
      });
    }
    const tailOffset = 90084;
    const tail = ht.subarray(tailOffset);
    let pos = 0;
    const numMatrices = tail.readUInt32LE(pos);
    pos += 4;
    const matrices = [];
    for (let m = 0; m < numMatrices; m++) {
      const sz = tail.readUInt32LE(pos);
      pos += 4;
      const matBytes = Array.from(tail.subarray(pos, pos + sz));
      pos += sz;
      const dim = Math.round(Math.sqrt(sz));
      matrices.push({
        index: m,
        sizeBytes: sz,
        dimensions: `${dim}x${dim}`,
        minVal: Math.min(...matBytes),
        maxVal: Math.max(...matBytes),
        sampleRow: matBytes.slice(0, dim),
        values: matBytes,
      });
    }
    return {
      totalBytes: ht.length,
      sha256: crypto.createHash('sha256').update(ht).digest('hex').slice(0, 16),
      tables,
      matrices,
    };
  } catch {
    return null;
  }
}

async function startServer() {
  const app = express();
  const PORT = 3000;
  app.use(express.json());

  // Stream authentic Canon voice alert WAV files from /assets/sounds
  app.get('/api/sounds/:filename', (req, res) => {
    const safeName = path.basename(req.params.filename);
    const soundPath = path.join(__dirname, 'assets', 'sounds', safeName);
    if (!fs.existsSync(soundPath)) {
      res.status(404).json({ error: 'Sound file not found' });
      return;
    }
    res.setHeader('Content-Type', 'audio/wav');
    fs.createReadStream(soundPath).pipe(res);
  });

  // Serve real binary & source repository audit
  app.get('/api/repository-audit', (_req, res) => {
    const cap3ht = parseCap3htBinary(path.join(__dirname, 'assets', 'Cap3ht.bin'));

    const xpDriverDir = path.join(__dirname, 'reference', 'drivers', 'LBP1120_WinXP');
    const dllMetadata: {
      name: string;
      role: string;
      sizeBytes: number;
      sha256Short: string;
      exports: string[];
    }[] = [];

    const roleDescriptions: Record<string, string> = {
      'CAP3K.dll': 'Windows NT/XP GDI Graphics Rendering Driver (DrvEnableDriver)',
      'CAP3UIK.dll': 'Printer Property Sheets & DEVMODE UI Configuration DLL',
      'CAP3RDN.dll': 'Host Rendering Engine (HRE) & SCoA Compression Library',
      'cap3emn.dll': 'CAPT 1.0 Engine Manager & Opcode State Machine',
      'CAP3LMK.DLL': 'Windows Spooler Language Monitor (Bidirectional Pipe Gate)',
      'cap3ptmn.dll': 'USB / Parallel Port Monitor Interface',
      'CAP3SMK.DLL': 'Status Monitor Client/Server RPC Shared Library',
      'CAP3PMN.dll': 'Printer Status Window UI & Audio Alert Controller',
      'CAP3RSK.exe': 'Background CAPT Spooler & RPC Daemon Executable',
      'cap3laK.exe': 'Canon LASER SHOT Status Window GUI Application',
    };

    if (fs.existsSync(xpDriverDir)) {
      for (const [fileName, role] of Object.entries(roleDescriptions)) {
        const fullPath = path.join(xpDriverDir, fileName);
        if (fs.existsSync(fullPath)) {
          const buf = fs.readFileSync(fullPath);
          dllMetadata.push({
            name: fileName,
            role,
            sizeBytes: buf.length,
            sha256Short: crypto.createHash('sha256').update(buf).digest('hex').slice(0, 12),
            exports: parsePeExports(fullPath),
          });
        }
      }
    }

    const soundsDir = path.join(__dirname, 'assets', 'sounds');
    const soundLabels: Record<string, string> = {
      'Cap3strt.wav': 'Print Job Started (PrintStart)',
      'Cap3done.wav': 'Print Completed Successfully (PrintComplete)',
      'Cap3addp.wav': 'Paper Tray Empty — Load Paper (AddPaper)',
      'Cap3come.wav': 'Paper Loaded / Check Printer (CheckPrinter)',
      'Cap3covr.wav': 'Top Door / Cover Open (CoverOpen)',
      'Cap3jam.wav': 'Paper Jam Detected in Path (PaperJam)',
      'Cap3cart.wav': 'Toner Cartridge Missing (NoCartridge)',
      'Cap3err.wav': 'General Printer Fault (PrinterError)',
      'Cap3mem.wav': '512KB Onboard Memory Full (LowMemory)',
      'Cap3cln.wav': 'Fuser Roller Cleaning Active (Cleaning)',
      'Cap3paus.wav': 'Print Job Paused (PrintPause)',
      'Cap3resm.wav': 'Print Job Resumed (PrintResume)',
      'Cap3stop.wav': 'Print Job Stopped (PrintStop)',
      'Cap3chgp.wav': 'Paper Size Mismatch (ChangePaper)',
      'Cap3serr.wav': 'Spool Server Communication Error (CheckServer)',
      'Cap3potb.wav': 'USB Port Busy (PortBusy)',
      'Cap3fail.wav': 'Print Job Failed (PrintFail)',
    };

    const sounds = fs.existsSync(soundsDir)
      ? fs
          .readdirSync(soundsDir)
          .filter((f) => f.toLowerCase().endsWith('.wav'))
          .map((f) => {
            const st = fs.statSync(path.join(soundsDir, f));
            return {
              filename: f,
              label: soundLabels[f] || f,
              sizeBytes: st.size,
            };
          })
      : [];

    const sourceFiles = [
      'src/protocol.rs',
      'src/scoa.rs',
      'src/halftone.rs',
      'src/job_manager.rs',
      'src/usb.rs',
      'src/spool.rs',
      'src/pdf.rs',
      'src/pbm.rs',
      'src/a4.rs',
      'src/sound.rs',
      'src/main.rs',
      'docs/CAPT_PROTOCOL_SPECIFICATION.md',
      'docs/ARCHITECTURE_AND_ROADMAP.md',
      'linux/cups-capt1120/Canon-LBP-1120-capt.ppd',
      'installer/CanonLBP1120_Setup.iss',
      'reference/drivers/LBP1120_WinXP/Cap3setk.inf',
    ]
      .filter((rel) => fs.existsSync(path.join(__dirname, rel)))
      .map((rel) => {
        const content = fs.readFileSync(path.join(__dirname, rel), 'utf8');
        return {
          path: rel,
          lines: content.split('\n').length,
          bytes: Buffer.byteLength(content, 'utf8'),
        };
      });

    res.json({
      cap3ht,
      dllMetadata,
      sounds,
      sourceFiles,
    });
  });

  // Endpoint to read any allowed project source file for live inspection
  app.get('/api/source-file', (req, res) => {
    const relPath = String(req.query.path || '');
    const allowedPrefixes = [
      'src/',
      'docs/',
      'linux/',
      'installer/',
      'reference/drivers/LBP1120_WinXP/Cap3setk.inf',
      'reference/drivers/LBP1120_WinXP/Cap3infk.dat',
      'Cargo.toml',
      'README.md',
    ];
    if (
      relPath.includes('..') ||
      !allowedPrefixes.some((prefix) => relPath === prefix || relPath.startsWith(prefix))
    ) {
      res.status(403).json({ error: 'Access denied to path' });
      return;
    }
    const fullPath = path.join(__dirname, relPath);
    if (!fs.existsSync(fullPath)) {
      res.status(404).json({ error: 'File not found' });
      return;
    }
    const content = fs.readFileSync(fullPath, 'utf8');
    res.json({ path: relPath, content });
  });

  if (process.env.NODE_ENV !== 'production') {
    const vite = await createViteServer({
      server: { middlewareMode: true },
      appType: 'spa',
    });
    app.use(vite.middlewares);
  } else {
    const distPath = path.join(__dirname, 'dist');
    app.use(express.static(distPath));
    app.get('*', (_req, res) => {
      res.sendFile(path.join(distPath, 'index.html'));
    });
  }

  app.listen(PORT, '0.0.0.0', () => {
    console.log(`Canon LBP-1120 Driver Ecosystem Workbench listening on http://0.0.0.0:${PORT}`);
  });
}

startServer();
