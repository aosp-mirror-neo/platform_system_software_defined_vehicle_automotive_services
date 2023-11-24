# SDV ConCal sample implementation

## Sample code generation

To generate code from the VSIDL definitions, run the following command from this folder:

```bash
vsidl_code_generator \
    --vsidl-path ../../../core_services/vsidl/syntax/ \
    --catalog-path ../../concal/vsidl/ \
    --output-path ./
```

If you get an error that *vsidl_code_generator* is not found, build it first:

```bash
mm vsidl_code_generator
```