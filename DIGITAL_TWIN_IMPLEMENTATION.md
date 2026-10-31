# DIGITAL TWIN IMPLEMENTATION SPECIFICATION
## Canon LaserShot LBP-1120 Virtual Hardware Subsystem

**Document ID:** `TWIN-IMPL-LBP1120-2026-10-31`  
**Classification:** Software Implementation Specification  
**Source Module:** `src/digital_twin.rs`

---

## 1. Module Overview & Public API

The Digital Twin is implemented natively in Rust within `src/digital_twin.rs`. It provides two primary components:

1. `VirtualCaptDevice`: An in-memory, thread-safe software emulator representing the physical Canon LaserShot LBP-1120.
2. `VirtualUsbTransport`: An implementation of the printer transport interface that routes bulk and control transactions directly into the `VirtualCaptDevice` without accessing WinUSB or physical USB hardware.

```rust
use crate::digital_twin::VirtualCaptDevice;

// Instantiate virtual device with default healthy state
let mut virtual_printer = VirtualCaptDevice::new();

// Simulate sensory conditions
virtual_printer.set_paper_empty(false);
virtual_printer.set_door_open(false);
virtual_printer.set_toner_present(true);

// Process raw CAPT packet bytes from host
let response = virtual_printer.handle_bulk_write(&packet_bytes)?;

// Inspect decoded 600 DPI bitmap after IC_END_PAGE
if let Some(canvas) = virtual_printer.last_printed_page() {
    canvas.save_pbm("simulated_page.pbm")?;
}
```

---

## 2. Emulated Command Handling Engine

The `VirtualCaptDevice` decodes all documented CAPT 1.0 opcodes:

| Opcode | Handler Routine | State Transitions & Side Effects |
| :--- | :--- | :--- |
| `0xE0A0` (`PCR_GET_BASIC_STATUS`) | `handle_get_basic_status()` | Returns 6 bytes: `[A0 E0 06 00 basic changed]`. Updates changed flags. |
| `0xA0A0` (`PC_GET_EXTENDED_STATUS`) | `handle_get_extended_status()` | Returns 20 bytes including door switch, toner presence, tray state, and counters. |
| `0xE0A5` (`PCR_GO_ONLINE`) | `handle_go_online(payload)` | Validates magic `0xADEADBEE`. Transitions engine from Standby to Pre-rotation. Clears offline bit (`basic &= ~0x10`). |
| `0xE0A6` (`PCR_GO_OFFLINE`) | `handle_go_offline()` | Returns engine to Standby. Sets offline bit (`basic |= 0x10`). |
| `0xE0A2` (`PCR_CLEAR_ERROR`) | `handle_clear_error()` | Resets error latches (`basic &= ~0x80`). |
| `0xD0A0` (`IC_BEGIN_PAGE`) | `handle_begin_page(payload)` | Parses 34-byte page configuration (margins, DPI, line width). Allocates virtual frame canvas. |
| `0xD0A1` (`IC_BEGIN_DATA`) | `handle_begin_data()` | Opens SCoA decompressor gate. Enters strict streaming mode. |
| `0xC0A0` (`IC_VIDEO_DATA`) | `handle_video_data(chunk)` | Appends chunk to DRAM ring buffer. Streams payload into SCoA decompressor. |
| `0xD0A2` (`IC_END_PAGE`) | `handle_end_page()` | Finalizes SCoA stream. Simulates paper feed transit. Increments `printed_pages` counter. |

---

## 3. Verification & Automated Test Coverage

The digital twin includes dedicated test cases verifying:
1. Standard page lifecycle from Online reservation through SCoA streaming to page ejection.
2. Interruption handling: Rejecting print jobs when the virtual paper tray is empty or top cover is open.
3. Reconstruction fidelity: Verifying that synthetic vector patterns rendered to PDF and transmitted to the virtual device decode into a 100% bit-exact match of the original bitmap.
