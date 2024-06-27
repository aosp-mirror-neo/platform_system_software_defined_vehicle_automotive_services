# VePSM(Vehicle Power State Manager)

This is a sample binary for what the OEM should implement in order to set
vehicle/power states or listen to power state machine updates in the SDV VM.

The binary implements the RPC client VSIDL interface defined by SDV VPM (Vehicle
Mode and Power Management) agent to set the vehicle state.

```text
package: "sdv.vpm.vehicle.client.rpc"

sdv_service_bundle {
    name: "VpmSystemClient"
    client:  "sdv.vpm.vehicle.VpmSystemService"
    server:  "sdv.vpm.PowerNotificationService"
}
```

Middleware generated code is used for implementing the VSIDL interface and can
be found in the `./generated` folder. The code was generated using the
`vsidl_code_generator` as follows:

```bash
vsidl_code_generator --vsidl-path core_services/vsidl/protos/  --catalog-path automotive_services/vpm/vsidl/  --output-path automotive_services/vpm/
```

***Warning:*** The current APIs are not final and pending API council approval.

## Usage

To set the vehicle/power states in the SDV VM, first run the binary with the
following command, this will create the server for power state callbacks.

```bash
vepsm
```

Then there will be a prompt:

```bash
Enter VePSM command, press enter for help or cntl + c to exit!
```

```bash
Commands:
  vehicle-state  Vpm Vehicle state change command
  power-state    Vpm Power state change command
  help           Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

Additional help for vehicle states can be requested with:

```bash

vepsm vehicle-state --help

Usage: vepsm vehicle-state <COMMAND>

Commands:
  low-power
  software-update
  park
  life-on-board
  vehicle-on
  traction-on
  help             Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

or for power states

```bash

vepsm power-state --help

Usage: vepsm power-state <COMMAND>

Commands:
  power-on  Set the power state to on
  prepare   Prepare the VM for suspension or shutdown
  cancel    Cancel the preparation for suspension or shutdown
  finish    Turn the VM off
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

Some power states [prepare, finish] also have addition parameters and these can
be found via

```bash
vepsm power-state prepare --help

Usage: vepsm power-state prepare <PREPARE_TYPE>

Arguments:
  <PREPARE_TYPE>
          Type of preparation

          Possible values:
          - shut-down: Shutdown the VM
          - ram:       Suspend the VM to RAM
          - disk:      Suspend the VM to DISK

Options:
  -h, --help
```

To suspend the VM to RAM on Cuttelfish:

```bash
  adb root & adb shell
  vepsm power-state power-on
  vepsm power-state prepare ram
  vepsm power-state finish ram

# To wake up the VM from suspension, execute the following from the host.
 cvd powerbtn
```

To shutdown the VM on Cuttelfish:

```bash
  vepsm power-state power-on
  vepsm power-state prepare shut-down
  vepsm power-state finish shut-down
```
