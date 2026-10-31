# PRINTING SEQUENCE DIAGRAM & PACKET FLOW
## Microsecond-Level USB Packet Flow & Hardware Handshakes

**Document ID:** `SEQ-DIAG-LBP1120-2026-10-31`  
**Classification:** Protocol Interaction Sequence Diagram  
**Scope:** Verified on Physical Canon LaserShot LBP-1120

---

## 1. High-Precision USB Packet Sequence

```mermaid
sequenceDiagram
    autonumber
    participant Host as Host Spool Daemon (capt-poc)
    participant EP0 as Control Endpoint 0x00
    participant BulkOut as Bulk OUT Pipe 0x01
    participant BulkIn as Bulk IN Pipe 0x82
    participant HW as Physical Printer Hardware (LBP-1120)

    Note over Host,HW: Phase 1: Device Detection & Status Probe
    Host->>EP0: Class Request: GET_PORT_STATUS (0xA1, Req 1)
    EP0-->>Host: Returns byte 0x18 (Printer Selected / Hardware OK)

    Host->>BulkOut: Send 0xE0A0 (PCR_GET_BASIC_STATUS, 4B)
    BulkIn-->>Host: Reply 6B: [A0 E0 06 00] [basic] [changed]
    Note over Host: Verify: basic & 0x02 == 0 (Paper Loaded)<br/>Verify: basic & 0x80 == 0 (Not Busy)

    Host->>BulkOut: Send 0xA0A0 (PC_GET_EXTENDED_STATUS, 4B)
    BulkIn-->>Host: Reply 20B: [A0 A0 14 00 ...] (Check Door/Toner/Counters)

    Note over Host,HW: Phase 2: Engine Online Reservation
    Host->>BulkOut: Send 0xE0A5 (PCR_GO_ONLINE, 12B)<br/>Payload: [0xADEADBEE] [page_idx=0] [0x0000]
    BulkIn-->>Host: Reply 6B: [A0 E0 06 00] [basic=0x00] [changed=0x10]<br/>(Online bit cleared -> Printer is now Online!)

    Note over Host,HW: Phase 3: Page Geometry & Video Initialization
    Host->>BulkOut: Send 0xD0A0 (IC_BEGIN_PAGE, 38B)<br/>Model=0x01A4, Lines=6778, LineSize=592, Res=0x11
    Host->>BulkOut: Send 0xD0A1 (IC_BEGIN_DATA, 4B)<br/>(Opens SCoA hardware decompression engine)

    Note over Host,HW: Phase 4: SCoA Compressed Video Streaming
    loop Stream in 4,096-Byte Payloads (4,100B Packets)
        Host->>BulkOut: Send 0xC0A0 (IC_VIDEO_DATA, 4100B)
        Note over HW: ASIC streams chunk into 512 KB DRAM Ring FIFO
    end
    Host->>BulkOut: Send Final 0xC0A0 (IC_VIDEO_DATA with remaining bytes + EOP 0x42)

    Note over Host,HW: Phase 5: Page Execution & Paper Pickup Trigger
    Host->>BulkOut: Send 0xD0A2 (IC_END_PAGE, 4B)
    Note over HW: Engine fires Solenoid SL1<br/>Main motor starts pickup cycle<br/>Laser writes latent image onto drum<br/>Paper travels through transfer & fuser

    Note over Host,HW: Phase 6: Post-Printing Poll & Cooldown
    loop Poll Status until Page Ejected (Interval: 500ms)
        Host->>BulkOut: Send 0xA0A0 (PC_GET_EXTENDED_STATUS, 4B)
        BulkIn-->>Host: Reply 20B (Track printed_pages increment)
    end
    Note over Host: printed_pages incremented from N to N+1!<br/>Play Sound: Cap3done.wav (Print job completed)

    Note over Host,HW: Phase 7: Session Release
    Host->>BulkOut: Send 0xE0A6 (PCR_GO_OFFLINE, 4B)
    BulkIn-->>Host: Reply 6B: basic=0x10 (Offline returned)
```
