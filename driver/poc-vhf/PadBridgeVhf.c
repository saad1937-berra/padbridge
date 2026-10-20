// PadBridge VHF POC B — source KMDF experimental, NON TESTE / NON INSTALLABLE.
// VHF presente un gamepad HID generique Windows/DirectInput. Ceci n'est PAS XInput.
// Ce fichier exige Visual Studio, WDK, Wdf.h, Vhf.h, VhfKm.lib et un INF valide.
// Aucune installation sur la machine principale avant audit, tests de charge et VM.

#include <initguid.h>
#include <wdmsec.h>
#include "PadBridgeVhf.h"

// GUID prive, propre a PadBridge. Pas de faux VID/PID Microsoft, Sony ou Xbox.
DEFINE_GUID(GUID_DEVINTERFACE_PADBRIDGE_VHF,
    0xe23bd620, 0x6064, 0x4879, 0x9c, 0x10, 0x1a, 0x5d, 0x42, 0x2b, 0xcc, 0x21);

// Report ID (1), 11 buttons (2 bytes with 5 bits padding), D-pad hat (1 byte),
// signed 8-bit LX/LY/RX/RY (4), LT/RT (2) : 10 octets au total.
static UCHAR PbReportDescriptor[] = {
    0x05, 0x01,       // Usage Page: Generic Desktop
    0x09, 0x05,       // Usage: Game Pad
    0xA1, 0x01,       // Collection: Application
    0x85, 0x01,       //   Report ID 1

    0x05, 0x09,       //   Usage Page: Button
    0x19, 0x01,       //   Usage Minimum: 1
    0x29, 0x0B,       //   Usage Maximum: 11
    0x15, 0x00,       //   Logical Minimum: 0
    0x25, 0x01,       //   Logical Maximum: 1
    0x75, 0x01,       //   Report Size: 1
    0x95, 0x0B,       //   Report Count: 11
    0x81, 0x02,       //   Input: Data, Variable, Absolute
    0x75, 0x01,
    0x95, 0x05,       //   Pad remaining 5 bits of the 2-byte bitmap
    0x81, 0x03,       //   Input: Constant

    0x05, 0x01,       //   Usage Page: Generic Desktop
    0x09, 0x39,       //   Usage: Hat Switch
    0x15, 0x00,       //   Logical Minimum: 0
    0x25, 0x07,       //   Logical Maximum: 7
    0x35, 0x00,       //   Physical Minimum: 0
    0x46, 0x3B, 0x01, //   Physical Maximum: 315 degrees
    0x65, 0x14,       //   Unit: degrees
    0x75, 0x04,       //   Report Size: 4
    0x95, 0x01,
    0x81, 0x42,       //   Input: Data, Variable, Absolute, Null State (8)
    0x65, 0x00,       //   Reset unit
    0x75, 0x04,
    0x95, 0x01,
    0x81, 0x03,       //   Upper four hat bits are constant padding

    0x09, 0x30,       //   Usage X
    0x09, 0x31,       //   Usage Y
    0x09, 0x33,       //   Usage Rx
    0x09, 0x34,       //   Usage Ry
    0x15, 0x81,       //   Logical Minimum: -127
    0x25, 0x7F,       //   Logical Maximum: 127
    0x75, 0x08,
    0x95, 0x04,
    0x81, 0x02,

    0x09, 0x32,       //   Usage Z (LT)
    0x09, 0x35,       //   Usage Rz (RT)
    0x15, 0x00,
    0x26, 0xFF, 0x00, //   Logical Maximum: 255
    0x75, 0x08,
    0x95, 0x02,
    0x81, 0x02,
    0xC0,             // End Collection
};

typedef struct _PB_DEVICE_CONTEXT {
    VHFHANDLE VhfHandle;
} PB_DEVICE_CONTEXT, *PPB_DEVICE_CONTEXT;

WDF_DECLARE_CONTEXT_TYPE_WITH_NAME(PB_DEVICE_CONTEXT, PbGetContext);

EVT_WDF_DRIVER_DEVICE_ADD PbEvtDeviceAdd;
EVT_WDF_OBJECT_CONTEXT_CLEANUP PbEvtDeviceCleanup;
EVT_WDF_IO_QUEUE_IO_DEVICE_CONTROL PbEvtIoDeviceControl;
EVT_WDF_FILE_CLEANUP PbEvtFileCleanup;

_Use_decl_annotations_
NTSTATUS DriverEntry(PDRIVER_OBJECT DriverObject, PUNICODE_STRING RegistryPath)
{
    WDF_DRIVER_CONFIG config;
    WDF_DRIVER_CONFIG_INIT(&config, PbEvtDeviceAdd);
    return WdfDriverCreate(
        DriverObject, RegistryPath, WDF_NO_OBJECT_ATTRIBUTES, &config, WDF_NO_HANDLE);
}

_Use_decl_annotations_
VOID PbEvtDeviceCleanup(WDFOBJECT Object)
{
    // Appele par KMDF au PASSIVE_LEVEL grace a ExecutionLevelPassive.
    PPB_DEVICE_CONTEXT context = PbGetContext((WDFDEVICE)Object);
    if (context->VhfHandle != NULL) {
        VhfDelete(context->VhfHandle, TRUE);
        context->VhfHandle = NULL;
    }
}

// Si le client unique ferme son handle (y compris apres un crash), retirer
// les commandes maintenues. Un watchdog temporel est encore requis avant VM.
_Use_decl_annotations_
VOID PbEvtFileCleanup(WDFFILEOBJECT FileObject)
{
    PPB_DEVICE_CONTEXT context = PbGetContext(WdfFileObjectGetDevice(FileObject));
    PB_VHF_REPORT neutral = { 0 };
    HID_XFER_PACKET packet = { 0 };

    if (context->VhfHandle == NULL) return;
    neutral.ReportId = PB_VHF_REPORT_ID;
    neutral.Hat = PB_VHF_HAT_NEUTRAL;
    packet.reportId = PB_VHF_REPORT_ID;
    packet.reportBuffer = (PUCHAR)&neutral;
    packet.reportBufferLen = (ULONG)sizeof(neutral);
    (VOID)VhfReadReportSubmit(context->VhfHandle, &packet);
}

_Use_decl_annotations_
VOID PbEvtIoDeviceControl(
    WDFQUEUE Queue,
    WDFREQUEST Request,
    size_t OutputBufferLength,
    size_t InputBufferLength,
    ULONG IoControlCode)
{
    NTSTATUS status = STATUS_INVALID_DEVICE_REQUEST;
    PVOID buffer = NULL;
    PPB_DEVICE_CONTEXT context = PbGetContext(WdfIoQueueGetDevice(Queue));
    HID_XFER_PACKET packet = { 0 };
    PB_VHF_REPORT report;

    UNREFERENCED_PARAMETER(OutputBufferLength);

    if (IoControlCode != PB_VHF_IOCTL_SUBMIT) {
        WdfRequestComplete(Request, status);
        return;
    }
    if (InputBufferLength != sizeof(PB_VHF_REPORT)) {
        WdfRequestComplete(Request, STATUS_INVALID_BUFFER_SIZE);
        return;
    }

    status = WdfRequestRetrieveInputBuffer(Request, sizeof(PB_VHF_REPORT), &buffer, NULL);
    if (!NT_SUCCESS(status)) {
        WdfRequestComplete(Request, status);
        return;
    }

    RtlCopyMemory(&report, buffer, sizeof(report));
    if (report.ReportId != PB_VHF_REPORT_ID ||
        (report.Buttons & ~PB_VHF_VALID_BUTTONS) != 0 ||
        report.Hat > PB_VHF_HAT_NEUTRAL ||
        report.Lx == -128 || report.Ly == -128 ||
        report.Rx == -128 || report.Ry == -128) {
        WdfRequestComplete(Request, STATUS_INVALID_PARAMETER);
        return;
    }
    if (context->VhfHandle == NULL) {
        WdfRequestComplete(Request, STATUS_DEVICE_NOT_READY);
        return;
    }

    // VHF utilise son tampon par defaut. Le rapport local est reutilisable
    // des le retour de VhfReadReportSubmit.
    packet.reportId = PB_VHF_REPORT_ID;
    packet.reportBuffer = (PUCHAR)&report;
    packet.reportBufferLen = (ULONG)sizeof(report);
    status = VhfReadReportSubmit(context->VhfHandle, &packet);
    WdfRequestComplete(Request, status);
}

_Use_decl_annotations_
NTSTATUS PbEvtDeviceAdd(WDFDRIVER Driver, PWDFDEVICE_INIT DeviceInit)
{
    NTSTATUS status;
    WDFDEVICE device;
    WDF_OBJECT_ATTRIBUTES attributes;
    WDF_IO_QUEUE_CONFIG queueConfig;
    WDF_FILEOBJECT_CONFIG fileConfig;
    VHF_CONFIG vhfConfig;
    PPB_DEVICE_CONTEXT context;
    UNICODE_STRING name = RTL_CONSTANT_STRING(L"\\Device\\PadBridgeVhfPoc");

    UNREFERENCED_PARAMETER(Driver);

    // Ne jamais exposer un mecanisme d'injection de controles a tout utilisateur.
    // L'interface est limitee au SYSTEME et aux administrateurs.
    status = WdfDeviceInitAssignName(DeviceInit, &name);
    if (!NT_SUCCESS(status)) return status;

    status = WdfDeviceInitAssignSDDLString(DeviceInit, &SDDL_DEVOBJ_SYS_ALL_ADM_ALL);
    if (!NT_SUCCESS(status)) return status;

    // Un seul broker pilote l'etat virtuel. L'INF devra aussi rendre
    // le PDO exclusif pour garantir cela a l'echelle de la pile.
    WdfDeviceInitSetExclusive(DeviceInit, TRUE);
    WDF_FILEOBJECT_CONFIG_INIT(
        &fileConfig, WDF_NO_EVENT_CALLBACK, WDF_NO_EVENT_CALLBACK, PbEvtFileCleanup);
    WdfDeviceInitSetFileObjectConfig(DeviceInit, &fileConfig, WDF_NO_OBJECT_ATTRIBUTES);

    WDF_OBJECT_ATTRIBUTES_INIT_CONTEXT_TYPE(&attributes, PB_DEVICE_CONTEXT);
    attributes.EvtCleanupCallback = PbEvtDeviceCleanup;
    attributes.ExecutionLevel = WdfExecutionLevelPassive;

    status = WdfDeviceCreate(&DeviceInit, &attributes, &device);
    if (!NT_SUCCESS(status)) return status;

    context = PbGetContext(device);
    context->VhfHandle = NULL;

    WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE(&queueConfig, WdfIoQueueDispatchSequential);
    queueConfig.EvtIoDeviceControl = PbEvtIoDeviceControl;
    status = WdfIoQueueCreate(device, &queueConfig, WDF_NO_OBJECT_ATTRIBUTES, WDF_NO_HANDLE);
    if (!NT_SUCCESS(status)) return status;

    status = WdfDeviceCreateDeviceInterface(device, &GUID_DEVINTERFACE_PADBRIDGE_VHF, NULL);
    if (!NT_SUCCESS(status)) return status;

    // Ce sont des identifiants internes au prototype, pas des identifiants Xbox.
    VHF_CONFIG_INIT(
        &vhfConfig,
        WdfDeviceWdmGetDeviceObject(device),
        (USHORT)sizeof(PbReportDescriptor),
        PbReportDescriptor);
    // Aucun VID/PID d'un tiers ne doit etre usurpe. Utiliser l'absence
    // de valeurs explicites jusqu'a l'attribution d'identifiants legitimes.
    vhfConfig.VersionNumber = 0x0001;

    status = VhfCreate(&vhfConfig, &context->VhfHandle);
    if (!NT_SUCCESS(status)) return status;
    return VhfStart(context->VhfHandle);
}
