# Zana Completion Specification

> Historical note: this document is a planning artifact from earlier Zana/Zana
> development. It is preserved for context, not as the current release plan.

## Multi-Agent Parallel Development Plan

**Objective**: Complete Zana speech-to-text application with GPU orb visualizations

**Timeline**: Complete end-to-end in single session via parallel agents

**Current State**: Foundation complete (audio, STT, hooks, plugins), GUI structure exists but needs integration

---

## Phase 1: Foundation Integration (Parallel: 4 agents)

### Agent 1A: OrbRenderer wgpu Implementation
**File**: `src/gui/orb.rs` (currently 43 lines placeholder)
**Dependencies**: None

**Tasks**:
1. Read `plugins/nebula-aura-gpu/src/shaders/nebula.wgsl` (351 lines)
2. Study wgpu initialization patterns in `src/plugins/gpu.rs`
3. Implement OrbRenderer struct with:
   - wgpu Device, Queue, RenderPipeline fields
   - Shader module loading from nebula.wgsl
   - Vertex buffer (quad covering screen)
   - Uniform buffer for audio data (FFT bins, audio level, time)
   - Bind group layout and bind group
4. Implement `new()` method:
   - Initialize wgpu device and queue
   - Load shader from WGSL file
   - Create render pipeline with vertex/layout/fragment stages
   - Create buffers and bind groups
5. Implement `render()` method:
   - Update uniform buffer with current audio data
   - Encode render commands
   - Draw fullscreen quad
6. Implement `update_audio()` method to receive FFT + level data
7. Add error handling for wgpu initialization failures

**TDD**: Write unit tests for buffer creation, shader loading validation

**Deliverable**: Fully functional OrbRenderer that renders nebula shader to screen

---

### Agent 1B: GUI-Async Channel Bridge
**File**: `src/gui/app.rs` (currently 382 lines)
**Dependencies**: None

**Tasks**:
1. Create channel structure for async-to-sync communication:
   ```rust
   struct GuiChannels {
       recording_tx: mpsc::Sender<RecordingCommand>,
       recording_rx: mpsc::Receiver<RecordingEvent>,
       transcription_tx: mpsc::Sender<TranscriptionCommand>,
       transcription_rx: mpsc::Receiver<TranscriptionEvent>,
   }
   ```
2. Implement RecordingCommand/RecordingEvent enums (Start, Stop, AudioData)
3. Implement TranscriptionCommand/TranscriptionEvent enums (Transcribe, Result, Error)
4. Create background task spawner that runs async operations
5. Replace placeholder button handlers (lines 282-285, 299-302) with channel sends
6. Add polling logic in `update()` to receive events and update state
7. Ensure thread-safe communication with Arc<Mutex> where needed

**TDD**: Write tests for channel send/receive, command serialization

**Deliverable**: Working async bridge between GUI and core functionality

---

### Agent 1C: Settings Panel Complete
**File**: `src/gui/settings.rs` (currently 75 lines)
**Dependencies**: None

**Tasks**:
1. Add SettingsState struct with current settings from `src/state.rs`
2. Implement audio device selector:
   - Get device list from audio capture system
   - Display available devices in dropdown
   - Save selected device to settings
3. Implement Whisper model selector:
   - List: Tiny, Base, Small, Medium, Large-v3
   - Show current model
   - Check model download status
   - Download button if not present
4. Implement orb style selector:
   - Read available GPU plugins
   - Dropdown of styles (purple, cyan, fire, aurora, cosmic)
   - Pass selected style to OrbRenderer
5. Implement save button functionality:
   - Validate settings
   - Save to disk via state.rs
   - Show success/error message
6. Load settings on startup and populate UI
7. Add visual feedback for unsaved changes

**TDD**: Write tests for settings serialization, device selection validation

**Deliverable**: Complete settings UI with load/save/validate functionality

---

### Agent 1D: Event System GUI Integration
**File**: `src/gui/app.rs` + new `src/gui/event_handler.rs`
**Dependencies**: None

**Tasks**:
1. Create `GuiEventHandler` that subscribes to EventBus events
2. Subscribe to relevant events:
   - AudioLevelUpdated → update orb audio level
   - AudioFftUpdated → update orb FFT data
   - TranscriptionProgress → update progress UI
   - TranscriptionComplete → display transcription text
   - ErrorOccurred → show error dialog
3. Create thread-safe event queue for GUI updates
4. Process events in `ZanaApp::update()` method
5. Update UI components based on events:
   - Show "Recording..." status
   - Display real-time audio level
   - Update orb visualization with FFT data
   - Show transcription results in text panel
6. Handle edge cases (event overflow, stale data)

**TDD**: Write tests for event subscription, event queue processing

**Deliverable**: GUI that responds to hook system events in real-time

---

## Phase 2: Core Integration (Sequential: depends on Phase 1)

### Agent 2: Main Integration (Sequential)
**File**: `src/main.rs` + `src/gui/app.rs`
**Dependencies**: Phase 1 complete

**Tasks**:
1. Read current `src/main.rs` to understand entry point
2. Integrate all Phase 1 components:
   - Initialize OrbRenderer with wgpu device
   - Set up GUI channels
   - Register GUI event handler
   - Load settings at startup
3. Connect audio capture to event system:
   - Ensure audio events are emitted on recording
   - Route audio data to OrbRenderer
4. Connect transcription to UI:
   - Show transcription results in text panel
   - Update progress during transcription
5. Test full flow: record → transcribe → display → animate
6. Handle error cases throughout
7. Clean up resources on shutdown

**TDD**: Write integration test for full recording-transcription flow

**Deliverable**: Working end-to-end application

---

## Phase 3: Plugin System & Testing (Parallel: 3 agents)

### Agent 3A: Plugin Loading
**File**: `src/plugins/mod.rs` + new `src/plugins/loader.rs`
**Dependencies**: Phase 1 complete

**Tasks**:
1. Read current plugin infrastructure (`src/plugins/` directory)
2. Implement plugin discovery:
   - Scan `plugins/` directory for manifests
   - Load plugin metadata (name, type, dependencies)
   - Validate plugin compatibility
3. Implement dynamic plugin loading:
   - Load GPU plugins at startup
   - Instantiate plugin instances
   - Register plugins in PluginRegistry
4. Connect plugin selection in settings to actual loaded plugins
5. Handle plugin errors gracefully
6. Add logging for plugin loading status

**TDD**: Write tests for plugin discovery, loading, validation

**Deliverable**: Working plugin system that loads GPU visualization plugins

---

### Agent 3B: GUI Unit Tests
**File**: `src/gui/tests.rs` (new file)
**Dependencies**: Phase 1 complete

**Tasks**:
1. Write comprehensive GUI tests:
   - OrbRenderer buffer creation tests
   - Shader loading validation tests
   - Channel communication tests
   - Event handler subscription tests
   - Settings serialization tests
   - UI state management tests
2. Mock wgpu device for tests
3. Test error handling paths
4. Test concurrent access patterns
5. Achieve >80% code coverage for GUI module

**Deliverable**: Complete GUI test suite

---

### Agent 3C: Integration Tests
**File**: `tests/integration.rs` (new file)
**Dependencies**: Phase 2 complete

**Tasks**:
1. Write end-to-end integration tests:
   - Test: Record audio → emit events → orb updates
   - Test: Transcribe audio → display results
   - Test: Change settings → persist → reload
   - Test: Load plugin → select style → render
   - Test: Error handling (missing model, no audio device)
2. Test real audio capture (if device available)
3. Test transcription with actual whisper model (if available)
4. Test plugin loading and switching
5. Add benchmarks for critical paths

**Deliverable**: Complete integration test suite

---

## Phase 4: Documentation & Polish (Parallel: 2 agents)

### Agent 4A: Documentation Updates
**File**: Update `docs/ARCHITECTURE.md`, `docs/PLUGIN_DEVELOPMENT.md`
**Dependencies**: Phase 3 complete

**Tasks**:
1. Update architecture docs with GUI integration
2. Document wgpu rendering pipeline
3. Create user guide:
   - How to record audio
   - How to select models
   - How to change orb styles
   - Troubleshooting
4. Update plugin development guide:
   - How to create GPU visualization plugins
   - Plugin API reference
   - Shader development guide
5. Add inline code documentation
6. Create API documentation for public interfaces

**Deliverable**: Complete, up-to-date documentation

---

### Agent 4B: Final Polish & Cross-Platform Testing
**File**: All files
**Dependencies**: Phase 3 complete

**Tasks**:
1. Fix all 55 compilation warnings
2. Run clippy and address linter suggestions
3. Test on macOS (current platform):
   - Test transparent window support
   - Verify audio capture permissions
4. Prepare for Linux testing:
   - Check audio backend compatibility
   - Verify wgpu surface creation
5. Prepare for Windows testing:
   - Check audio backend compatibility
   - Verify shader compilation
6. Add logging for debugging
7. Performance profiling:
   - Identify bottlenecks
   - Optimize critical paths
   - Reduce allocations in hot paths
8. Error message improvements
9. UX polish (loading indicators, error dialogs)

**Deliverable**: Production-ready application with clean codebase

---

## Success Criteria

Application is complete when:
- [x] Code builds without errors
- [ ] All tests pass (unit + integration)
- [ ] Record button captures audio
- [ ] Orb animates with audio visualization
- [ ] Transcribe button processes audio and shows text
- [ ] Settings panel persists changes
- [ ] Plugin loading works
- [ ] Zero compilation warnings
- [ ] Documentation is current
- [ ] Cross-platform compatible

---

## Development Commands

```bash
# Phase 1: Launch 4 agents in parallel
tglm Phase1A_OrbRenderer "Implement OrbRenderer wgpu integration per spec"
tglm Phase1B_AsyncBridge "Implement GUI-async channel bridge per spec"
tglm Phase1C_Settings "Complete settings panel with selectors per spec"
tglm Phase1D_EventInt "Integrate event system with GUI per spec"

# Monitor progress
tlist
tcapture Phase1A_OrbRenderer 50
tcapture Phase1B_AsyncBridge 50
tcapture Phase1C_Settings 50
tcapture Phase1D_EventInt 50

# Phase 2: After Phase 1 complete
tglm Phase2_Integration "Integrate all components end-to-end per spec"

# Phase 3: Launch 3 agents in parallel
tglm Phase3A_PluginLoader "Implement plugin loading system per spec"
tglm Phase3B_GuiTests "Write GUI unit tests per spec"
tglm Phase3C_IntegrationTests "Write integration tests per spec"

# Phase 4: Launch 2 agents in parallel
tglm Phase4A_Documentation "Update all documentation per spec"
tglm Phase4B_Polish "Final polish, cross-platform testing per spec"
```

---

## Testing Strategy

**TDD Approach**:
1. Write failing test first
2. Implement minimum code to pass
3. Verify test passes
4. Refactor if needed
5. Move to next feature

**Test Types**:
- Unit tests: Individual functions and structs
- Integration tests: Multi-component workflows
- Property-based tests: Randomized testing for edge cases
- Manual tests: Real audio, real transcription

**Coverage Goals**:
- GUI module: >80%
- Audio module: >70% (already has tests)
- STT module: >70% (already has tests)
- Overall: >75%

---

## Key Files Reference

**Entry Points**:
- `src/main.rs` - Application entry point
- `src/gui/app.rs` - Main egui application

**GUI Components**:
- `src/gui/orb.rs` - Orb visualization renderer (NEEDS IMPLEMENTATION)
- `src/gui/settings.rs` - Settings panel (NEEDS COMPLETION)
- `src/gui/event_handler.rs` - Event system bridge (NEW)

**Core Systems**:
- `src/audio/capture.rs` - Audio capture (COMPLETE)
- `src/stt/whisper.rs` - Whisper transcription (COMPLETE)
- `src/hooks/` - Event system (COMPLETE)
- `src/plugins/` - Plugin framework (COMPLETE, needs loader)

**State & Config**:
- `src/state.rs` - Settings persistence (COMPLETE)

**Shaders**:
- `plugins/nebula-aura-gpu/src/shaders/nebula.wgsl` - Nebula shader (COMPLETE)

**Tests**:
- `src/gui/tests.rs` - GUI unit tests (NEW)
- `tests/integration.rs` - Integration tests (NEW)

**Documentation**:
- `docs/ARCHITECTURE.md` - Architecture docs (NEEDS UPDATE)
- `docs/PLUGIN_DEVELOPMENT.md` - Plugin guide (NEEDS UPDATE)
- `docs/USER_GUIDE.md` - User guide (NEW)

---

## Notes

- All agents should follow TDD: write tests first, then implement
- Each agent should validate their work compiles before completing
- Use `cargo build`, `cargo test`, `cargo clippy` frequently
- Ask for help if blocked on dependencies
- Document any deviations from this spec
- Prioritize working code over perfect code
- Keep functions focused and modular
- Use proper error handling (Result, not unwrap)
- Thread safety is critical (use Arc, Mutex, channels properly)
