<!--
  Copyright (C) 2026 The Android Open Source Project

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

# Configuration and Calibration Service Bundle

This directory contains the updated interface for the Configuration and
Calibration service (VSIDL), which will deprecate the currently existing API
version and will be used by default, after the MW code uses the new CommStack
API. This new API uses the new VSIDL syntax, SDV errors and splits ConCal
interface in several unit types.
