# disk-monitoring Specification

## Purpose
Watch free space on the home volume and any added fixed volumes, classify it as Healthy, Low or Critical, notify the user at the right moments, and forecast when the disk will fill.

Source: `docs/reference/FREEBOARD_SPEC.md`, Disk status, Forecast, Background behavior, Milestone 2.

## Requirements

### Requirement: Free-space sampling
Freeboard SHALL sample free and total bytes of each monitored volume every 5 minutes in memory, and store one sample per volume each hour. The home volume SHALL be monitored by default, and the user MAY add other fixed volumes.

#### Scenario: Hourly persistence
- **WHEN** a fake clock advances 3 hours with sampling every 5 minutes
- **THEN** 36 samples were taken and 3 were stored for each monitored volume

### Requirement: Status with hysteresis
Status SHALL be Healthy at or above the Low threshold, Low below it, and Critical below the Critical threshold (defaults 20 GB and 8 GB). A status SHALL improve only after free space rises 2 GB above the threshold it crossed.

#### Scenario: Boundaries
- **WHEN** free space is exactly 20.0 GB, then 19.9 GB, then 7.9 GB, with default thresholds
- **THEN** status is Healthy, then Low, then Critical

#### Scenario: Oscillation around a threshold
- **WHEN** free space moves between 19.5 GB and 21.5 GB repeatedly after entering Low
- **THEN** status stays Low until free space reaches 22 GB

### Requirement: Notifications
Freeboard SHALL notify once on entering Low, and on entering Critical then every 6 hours while it stays Critical. Clicking a notification SHALL open Freeboard on its home view. Notifications SHALL respect the Notifications setting and Pause Notifications.

#### Scenario: Critical reminders
- **WHEN** a fake clock keeps the volume Critical for 13 hours
- **THEN** exactly 3 Critical notifications are sent

#### Scenario: Paused
- **WHEN** notifications are paused and the volume enters Low
- **THEN** no notification is sent

### Requirement: Forecast
Freeboard SHALL compute daily change with a linear fit over the last 14 days of samples, excluding jumps caused by cleanups. It SHALL show "About N days left" only when at least 3 days of samples exist and free space is falling, and SHALL show no forecast otherwise.

#### Scenario: Not enough history
- **WHEN** only 2 days of samples exist
- **THEN** no forecast is shown

#### Scenario: Cleanup jump excluded
- **WHEN** samples fall steadily and include a 30 GB rise from a recorded cleanup
- **THEN** the fitted daily change matches the steady fall, not the jump

### Requirement: Tray status
The tray icon SHALL show a neutral, amber or red ring for Healthy, Low or Critical, and the tray view SHALL show status as text, free space and the current pile total.

#### Scenario: Status change updates tray
- **WHEN** status changes from Healthy to Low
- **THEN** the tray icon switches to the amber ring and a `status_changed` event carries the volume, status and free bytes
