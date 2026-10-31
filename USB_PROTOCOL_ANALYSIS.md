# USB PROTOCOL ANALYSIS & PACKET STRUCTURE SPECIFICATION
## Canon LaserShot LBP-1120 Low-Level USB Forensics

**Document ID:** `USB-ANALYSIS-LBP1120-2026-10-31`  
**Classification:** USB 1.1 Transport Protocol Specification  
**Verification:** Live USBPcap/Wireshark captures against physical hardware (`04A9:262B`)

---

## 1. USB Bus Enumeration & Descriptors

The Canon LaserShot LBP-1120 enumerates as a Full Speed USB 1.10 device:

### 1.1 Device Descriptor (18 Bytes)
```text
bLength             : 0x12 (18 bytes)
bDescriptorType     : 0x01 (DEVICE)
bcdUSB              : 0x0110 (USB 1.10)
bDeviceClass        : 0x00 (Defined at Interface level)
bDeviceSubClass     : 0x00
bDeviceProtocol     : 0x00
bMaxPacketSize0     : 0x40 (64 bytes)
idVendor            : 0x04A9 (Canon Inc.)
idProduct           : 0x262B (Canon LaserShot LBP-1120)
bcdDevice           : 0x0100 (Device Release 1.00)
iManufacturer       : 0x01 ("Canon")
iProduct            : 0x02 ("LASER SHOT LBP-1120")
iSerialNumber       : 0x03 (Factory unique hardware serial)
bNumConfigurations  : 0x01
```

### 1.2 Configuration & Interface Descriptors
- **Configuration 1:** 1 Interface, Bus Powered / Self Powered. Max Power: 100 mA.
- **Interface 0 (Printer Class):**
  - `bInterfaceClass`   : `0x07` (Printer Class)
  - `bInterfaceSubClass`: `0x01` (Printers)
  - `bInterfaceProtocol`: `0x02` (Bi-directional interface)
- **Endpoint 1 (Bulk OUT):**
  - Address: `0x01`
  - Attributes: Bulk (`0x02`)
  - MaxPacketSize: `64` bytes (`0x0040`)
  - Function: Streams all CAPT commands (`0xE0xx`, `0xA0xx`, `0xD0xx`) and compressed SCoA video chunks (`0xC0A0`).
- **Endpoint 2 (Bulk IN):**
  - Address: `0x82`
  - Attributes: Bulk (`0x02`)
  - MaxPacketSize: `64` bytes (`0x0040`)
  - Function: Receives 6-byte Basic Status and 20-byte Extended Status replies.

---

## 2. Control Endpoint 0 Class Requests

The printer supports standard USB Printer Class requests over Endpoint 0:

### 2.1 `GET_DEVICE_ID` (`0xA1`, Request `0x00`)
- **Setup Packet:** `bmRequestType = 0xA1`, `bRequest = 0x00`, `wValue = 0x0000`, `wIndex = 0x0000`, `wLength = 0x0100`.
- **Returned String (IEEE 1284 Device ID):**
  ```text
  MFG:Canon;MDL:LASER SHOT LBP-1120;CMD:CAPT;CLS:PRINTER;
  ```

### 2.2 `GET_PORT_STATUS` (`0xA1`, Request `0x01`)
- **Setup Packet:** `bmRequestType = 0xA1`, `bRequest = 0x01`, `wValue = 0x0000`, `wIndex = 0x0000`, `wLength = 0x0001`.
- **Returned Byte:** `0x18` (`Bit 4 = 1` Selected / Online; `Bit 3 = 1` No Error).
- **Critical Discovery:** On the USB bridge chip, this control byte is statically hardwired to `0x18` and *does not* reflect real-time paper tray or cover states. Dynamic status must always be queried via Bulk endpoints using `0xE0A0` and `0xA0A0`.

---

## 3. Bulk Packet Protocol Framing

Every CAPT 1.0 transaction over Bulk OUT (`0x01`) and Bulk IN (`0x82`) consists of a 4-byte Little-Endian header followed by payload:

```text
Byte 0..1 (u16 LE) : Opcode (e.g., 0xD0A0, 0xE0A0, 0xC0A0)
Byte 2..3 (u16 LE) : Total Packet Size including this 4-byte header (>= 4)
Byte 4..N          : Payload bytes (Total Packet Size - 4)
```

### 3.1 Status Query and Reply Packet Structure

```text
Host Query Packet (4 Bytes to Bulk OUT 0x01):
[0xA0] [0xE0] [0x04] [0x00]   --> PCR_GET_BASIC_STATUS (0xE0A0, Size: 4)

Device Response Packet (6 Bytes from Bulk IN 0x82):
[0xA0] [0xE0] [0x06] [0x00]   --> Opcode 0xE0A0, Size: 6
[basic] [changed]             --> Basic Register (Byte 4), Changed Register (Byte 5)
```

```text
Host Extended Query Packet (4 Bytes to Bulk OUT 0x01):
[0xA0] [0xA0] [0x04] [0x00]   --> PC_GET_EXTENDED_STATUS (0xA0A0, Size: 4)

Device Response Packet (20 Bytes from Bulk IN 0x82):
[0xA0] [0xA0] [0x14] [0x00]   --> Opcode 0xA0A0, Size: 20 (0x0014)
[basic] [changed]             --> Header Status Registers
[aux]                         --> Aux Register (Byte 6): 0x04 = Feed Solenoid, 0x02 = Engine Busy
[controller]                  --> Formatter state flags (Byte 7)
[paper_available_bits]        --> 0x80 = Tray has paper; 0x00 = Tray empty (Byte 8)
[reserved_0]                  --> 0x00 (Byte 9)
[engine_raw_lo] [engine_raw_hi] --> 0x4000 = Door open, 0x2000 = Cartridge missing (Bytes 10..11)
[printed_pages]               --> u8 Printed pages counter (Byte 12)
[printing_pages]              --> u8 Printing pages counter (Byte 13)
[shipped_pages]               --> u8 Shipped pages counter (Byte 14)
[start_pages]                 --> u8 Started pages counter (Byte 15)
[tail_bytes]                  --> 4 bytes reserved (Bytes 16..19)
```
