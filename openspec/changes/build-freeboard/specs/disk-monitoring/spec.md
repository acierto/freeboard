## Purpose

Warn developers of disk pressure without repeated threshold noise and provide a forecast supported by local measurements.

Source: `docs/reference/FREEBOARD_SPEC.md`, Cleanup and monitoring.

## ADDED Requirements

### Requirement: Volume sampling
The app SHALL monitor the home volume by default and user-added fixed volumes, sample every five minutes in memory, and persist one sample per hour.

#### Scenario: Scheduled sample
- **WHEN** a sampling interval elapses
- **THEN** each configured volume receives a free-byte and total-byte measurement and hourly persistence preserves 30 days of samples

### Requirement: Status and hysteresis
The app SHALL enter Low below 20 GB and Critical below 8 GB by default. Status SHALL improve only at 2 GB above the applicable threshold. Threshold settings SHALL control both status and gauge markers.

#### Scenario: Threshold oscillation
- **WHEN** free space enters Low and then fluctuates between 19 GB and 21 GB
- **THEN** status remains Low until it reaches 22 GB

#### Scenario: Critical recovery
- **WHEN** Critical free space rises to 9 GB and subsequently reaches 10 GB
- **THEN** it remains Critical at 9 GB and improves to Low at 10 GB

### Requirement: Notifications
The app SHALL notify once on entering Low or Critical, repeat Critical every six hours while unchanged, and open Suggestions when a notification is clicked. Healthy SHALL not notify; paused or disabled notifications SHALL be suppressed.

#### Scenario: Sustained critical status
- **WHEN** the disk stays Critical for six hours after its entry notification
- **THEN** one reminder is sent and ordinary samples do not generate additional notifications

### Requirement: Evidence-based forecast
The forecast SHALL use a linear fit over the last 14 days, exclude cleanup jumps, and show approximate days left only with at least three days of samples and falling free space.

#### Scenario: Insufficient or non-declining data
- **WHEN** fewer than three days of samples exist or the fitted free-space trend is flat or rising
- **THEN** no days-left forecast is displayed

### Requirement: Background scheduling
Monitoring and scheduled quick scans SHALL continue without the webview. Deep scans SHALL run at low priority and pause on battery by default.

#### Scenario: Battery transition
- **WHEN** an active deep scan encounters battery power with default settings
- **THEN** it pauses while lightweight monitoring remains active
