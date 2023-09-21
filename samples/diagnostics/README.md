# Diagnostics manager-service interaction sample

Sample that shows:
-how to implement and register diagnostic manager interfaces
-how to retrieve and call diagnostics manager from the server side

## Diagnostics manager

The diagnostics manager is an "empty" implementation of the interface that respond with a
generic diagnostics error code on all requests.

## Diagnostics service

The diagnostics service retrieves DiagnosticsManager from binder, calls methods on it and
examines the response.

## Example usage

1- Run the diagnostics manager:

```
diag_manager_sample
```

* Run the synchronous implementation:
  ```
  diag_manager_sample
  ```
* Run the asynchronous implementation:
  ```
  diag_manager_sample --async
  ```


2- Run the diagnostics service:
```
diag_service_sample --dm-host-cid 2 --dm-port 5750
```
