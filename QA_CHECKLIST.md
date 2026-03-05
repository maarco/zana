# Zana QA Release Checklist

## Pre-Release Testing

### Installation
- [ ] DMG opens correctly
- [ ] App drags to Applications folder
- [ ] App launches from Applications
- [ ] App launches from Spotlight

### First Run
- [ ] Onboarding screen appears on first launch
- [ ] Microphone permission prompt appears
- [ ] Accessibility permission prompt appears
- [ ] Whisper model downloads successfully

### Core Functionality
- [ ] Fn key triggers recording
- [ ] Orb appears during recording
- [ ] Orb responds to voice audio levels
- [ ] Release Fn key stops recording
- [ ] Transcription completes
- [ ] Text pastes at cursor position
- [ ] Double-tap Fn latches recording mode
- [ ] Single tap stops latched recording

### System Tray
- [ ] Tray icon appears in menu bar
- [ ] About menu opens About window
- [ ] Preferences menu opens Preferences window
- [ ] Quit menu exits application

### App Menu
- [ ] About Zana shows system about dialog
- [ ] Preferences opens settings
- [ ] Hide/Show All work correctly
- [ ] Quit exits application

### Preferences Window
- [ ] All settings display correctly
- [ ] Whisper model selection works
- [ ] Language selection works
- [ ] Toggle switches work
- [ ] Save button closes window

### About Window
- [ ] Version displays correctly
- [ ] Close button works

### Error Handling
- [ ] App handles missing microphone gracefully
- [ ] App handles missing accessibility gracefully
- [ ] Crash log written on panic

### Clean Installation Test
- [ ] Works on Mac without dev tools
- [ ] Works without Rust installed
- [ ] Works without Xcode installed

## Performance
- [ ] App starts quickly (< 3 seconds)
- [ ] Orb animation is smooth (60fps)
- [ ] Transcription completes in reasonable time
- [ ] No memory leaks during extended use

## Compatibility
- [ ] Works on macOS 10.15+
- [ ] Works on Apple Silicon
- [ ] Works on Intel Mac (if universal build)

## Final Checks
- [ ] Version number is correct in tauri.conf.json
- [ ] Copyright year is current
- [ ] No debug logs in release build
- [ ] Bundle ID is correct
- [ ] App icon displays correctly
- [ ] DMG background displays correctly

## Sign-off

Date: _______________
Tester: _______________
Version: _______________
Result: [ ] PASS  [ ] FAIL

Notes:
