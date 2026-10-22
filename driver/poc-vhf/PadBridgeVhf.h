// PadBridge VHF POC B / v0 — wire format only.
// Contrat versionne et intentionnellement simple, commun a Rust et KMDF.
// Aucun opcode d'installation, de vibration ou d'injection de clavier/souris.

#pragma once
#include <ntddk.h>
#include <wdf.h>
#include <vhf.h>

// Interface symbolique du pilote source (PAS de l'enfant HID virtuel).
// {E23BD620-6064-4879-9C10-1A5D422BCC21}
EXTERN_C const GUID GUID_DEVINTERFACE_PADBRIDGE_VHF;

#define PB_VHF_REPORT_ID 0x01
#define PB_VHF_HAT_NEUTRAL 0x08
#define PB_VHF_VALID_BUTTONS 0x07FFu
// Keep in sync with Rust vhf_protocol.rs. 200ms tick, 800ms inactivity timeout.
#define PB_WATCHDOG_INTERVAL_MS 200u
#define PB_WATCHDOG_TIMEOUT_MS 800u
#define PB_VHF_IOCTL_SUBMIT CTL_CODE(FILE_DEVICE_UNKNOWN, 0x800, METHOD_BUFFERED, FILE_WRITE_DATA)

#pragma pack(push, 1)
typedef struct _PB_VHF_REPORT {
    UCHAR ReportId;    // 0x01
    USHORT Buttons;    // little endian: A,B,X,Y,LB,RB,Back,Start,Guide,LS,RS (bits 0..10)
    UCHAR Hat;         // 0=Up,1=UpRight,...,7=UpLeft,8=neutral; high nibble always zero
    CHAR Lx;           // -127..127
    CHAR Ly;
    CHAR Rx;
    CHAR Ry;
    UCHAR Lt;          // 0..255
    UCHAR Rt;          // 0..255
} PB_VHF_REPORT, *PPB_VHF_REPORT;
#pragma pack(pop)

C_ASSERT(sizeof(PB_VHF_REPORT) == 10);
