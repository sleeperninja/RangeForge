# RangeForge - AI Context Transfer Document

## 🚨 CRITICAL: Read This First to Prevent Hallucinations

This document exists to **rapidly transfer project context to AI assistants** and prevent common misunderstandings that lead to scope creep and incorrect feature suggestions.

### 📚 **REQUIRED READING FOR FULL CONTEXT:**

**Start here, then read these for complete understanding:**

1. **AI_CONTEXT.md** (this file) - Scope restrictions and hallucination prevention
2. **PROJECT_PLAN.md** - Technical roadmap, current status, and architecture details
3. **VALUE_PROPOSITION.md** - User-facing messaging and core value proposition
4. **MARKET_RESEARCH.md** - User validation and competitive analysis
5. **DOCUMENT_CONSISTENCY.md** - Terminology standards and document status

**Quick Start Summary:** RangeForge consolidates 7+ audio tools into one volume balancing application. Phase 1 (preview workflow) complete, Phase 2 ("GO" button) is current priority. Single expert UI mode, personal productivity tool, avoid feature creep.

---

## ⚠️ COMMON AI HALLUCINATIONS TO AVOID

### ❌ **DO NOT SUGGEST THESE (Explicitly Rejected):**

- **Plugin architecture** - Single user project, no ecosystem needed
- **User-defined profiles** - UI complexity without value
- **iTunes/Plex integration** - No app integration, just compatible file output
- **Workflow scripting** - Would recreate PowerShell complexity being escaped
- **Quality assurance reports** - Not needed for personal workflow
- **Batch export monitoring** - Just visual progress on file list
- **Multiple UI modes** - Beginner/Intermediate modes removed, single expert interface only
- **User management** - Single user tool, no accounts or settings persistence
- **Cloud integration** - Local processing only, no cloud services

### ❌ **DO NOT ASSUME THESE:**

- Multiple users or team collaboration needs
- Enterprise deployment requirements
- Open source community development
- Plugin ecosystem or third-party extensions
- Integration with streaming services beyond Last.fm
- Complex user management or settings persistence

---

## ✅ **ACTUAL PROJECT CONTEXT**

### **WHO:** Single User (Power User Profile)

- **Primary user**: Project owner with video editing background
- **Secondary users**: Possible beta testers from similar background
- **Hardware**: High-end systems (64GB RAM, multi-core CPUs)
- **Technical level**: Comfortable with PowerShell scripting, professional audio concepts

### **WHAT:** Tool Consolidation Project

**Replaces 7+ existing tools:**

1. **Mediahuman Audio Converter** (CUE splitting)
2. **FFmpeg GUI** (format conversion)
3. **Adobe Audition** (problematic file handling)
4. **PowerShell script** (volume balancing)
5. **Mp3tag** (metadata management)
6. **Last.fm** (web lookups)
7. **Manual genre research** (web browsing)

**Into 1 unified application.**

### **WHY:** Specific Workflow Pain Points

- **Time inefficiency**: 45-75 minutes per album across 4+ apps
- **Loudness war discomfort**: Inconsistent volume hurts ears
- **Manual repetition**: Same metadata steps for every file
- **Context switching**: Jumping between applications breaks flow
- **Overnight processing**: PowerShell script too slow for interactive use

### **HOW:** Memory-Mapped Preview Architecture

- **NOT about memory efficiency** - about **workflow efficiency**
- **Strategy**: Pre-stage all operations for instant parameter tweaking
- **Benefit**: Change target LUFS, metadata, format without re-analysis
- **Cleanup**: Memory released per completed/errored file

---

## 📊 **ACTUAL USAGE PATTERNS**

### **File Processing Breakdown:**

- **93% Stereo Processing**: FLAC+CUE albums → ALAC (30%), Single files → ALAC (63%)
- **7% Multichannel**: 206GB curated collection (4-6 channel from SACD/DVD-A/BD-A)
- **1% Editing**: Convert for video editing workflow

### **Target Formats by Use Case:**

- **ALAC**: Primary output for iTunes/personal library (95% of usage)
- **FLAC**: Multichannel files for Plex compatibility, music collection
- **PCM/WAV**: Video editing proxies (fast scrubbing in DaVinci Resolve)
- **AAC**: Long files (3-4 hour recordings) for iTunes efficiency
- ❄️ **DD+**: ICEBOXED until FFmpeg encoding support matures

### **Professional Video Editing Overlap:**

- **Timeline audio**: PCM/WAV for fast scrubbing performance
- **Multichannel processing**: 4-6-8 channel workflows
- **Format flexibility**: DaVinci supports AIFF, MP3, AAC, FLAC on timeline
- **Batch processing**: Whole albums/discographies, occasional individual files### **Hardware Context:**

- **Main system**: 64GB RAM (can handle aggressive memory usage)
- **Target systems**: 16GB+ RAM minimum
- **Strategy**: Graceful degradation for lower-end systems

---

## 🎯 **VALIDATED MVP PROGRESSION**

### **Phase 1: ✅ COMPLETE**

- Preview-first workflow with memory-mapped file analysis
- CUE detection and virtual track creation
- Real-time FFmpeg LUFS/dBTP/DR analysis
- Table-based UI with progressive data population

### **Phase 2: 🎯 CURRENT PRIORITY**

**Infrastructure First:**

- **iSCSI migration** - Required for Electron binary compatibility
- **Development environment** - Network drive restrictions resolution

**Then Continue Development:**

- **"GO" button implementation** (commit previewed changes)
- Multi-format output (ALAC primary, FLAC, AAC, WAV)
- File naming strategies
- DSD/MLP input support (FFmpeg handles complexity)

### **Phase 3: 🎯 MULTICHANNEL WORKFLOW**

- 206GB collection processing (validated use case)
- 4-6 channel LUFS standards
- Multichannel → FLAC for Plex compatibility

### **Phase 4: 🎯 METADATA ENHANCEMENT**

- Last.fm API integration for genre tagging
- Manual genre editing in file list
- Album/Song/Chart level classification

---

## 🔧 **TECHNICAL ARCHITECTURE RATIONALE**

### **Why Electron + React?**

- **File system access** for large audio files
- **Desktop integration** for drag-drop workflow
- **Cross-platform** compatibility
- **NOT for web deployment** - desktop tool only

### **Why Memory Mapping (mmap)?**

- **Workflow efficiency** not memory efficiency
- **Pre-stage operations** for instant parameter adjustment
- **Eliminate redundant** decompress/recompress cycles
- **Target large files** (100-600MB common, up to 6GB albums)

### **Why Preview-First Architecture?**

- **User safety** - see changes before committing
- **Parameter tweaking** - adjust target LUFS without re-analysis
- **Workflow optimization** - make decisions with full information

### **Why FFmpeg Integration?**

- **Universal format support** - if FFmpeg can decode, app can process
- **Professional audio analysis** - LUFS/dBTP standards compliance
- **Existing expertise** - leverages proven audio processing

---

## 🎨 **UI/UX PRINCIPLES ESTABLISHED**

### **Core Design:**

- **Whole-window drop zone** - entire app accepts file drops
- **75/25 split** - file list dominates, controls secondary
- **Progressive disclosure** - Beginner/Intermediate/Expert modes
- **Asynchronous population** - data appears as analysis completes

### **Terminology Strategy:**

- **Beginner**: "Volume" not "LUFS", "Will Be Louder" not "+6dB"
- **Intermediate**: "Target Loudness (LUFS)" with context
- **Expert**: Full technical terminology

### **"Make It So" Philosophy:**

- **Pre-flight**: Analysis and preparation phase
- **"Make It So"**: Execution phase (Star Trek reference intentional)
- **File-by-file feedback**: Progress per track processing

---

## 🚫 **EXPLICITLY REJECTED APPROACHES**

### **Why No Plugin Architecture?**

- **Single user** - no plugin ecosystem to support
- **Complexity overhead** - maintenance burden without benefit
- **Scope creep risk** - infinite extensibility not needed

### **Why No User Profiles?**

- **UI complexity** without proportional value
- **Single workflow focus** - power user knows their preferences
- **Easy to recreate** settings manually when needed

**UPDATE**: User-defined presets are DIFFERENT from profiles - presets are user-created shortcuts for manual parameter combinations, not predefined workflow templates.

### **User Preset System (Phase 3)**

- **Manual controls**: User sets codec/rate/depth/channels manually
- **Save preset**: User creates named preset from current manual settings
- **Load preset**: Auto-fills manual controls, user can override
- **No smart presets**: Avoids development debt and inflexibility

### **Why No Integrations?**

- **File compatibility ≠ app integration**
- **iTunes/Plex compatibility** achieved through proper file formats
- **API integrations** limited to metadata only (Last.fm)

### **Why No Workflow Scripting?**

- **Escaping PowerShell complexity** - don't recreate it
- **GUI workflow preferred** - visual feedback and control
- **Batch processing** through file list, not scripting

---

## 📋 **SUCCESS METRICS DEFINED**

### **MVP1 Success**:

- Drop files → analyze → "GO" → perfect ALAC output
- **Eliminates**: PowerShell script from workflow

### **MVP2 Success**:

- In-app genre editing capabilities
- **Eliminates**: Mp3tag for basic workflow

### **MVP3 Success**:

- CUE splitting with metadata preservation
- **Eliminates**: Mediahuman Audio Converter

### **MVP4 Success**:

- Last.fm genre auto-population
- **Eliminates**: Manual web lookups

### **"Mission Accomplished" Scenario**:

> _"Drop Led Zeppelin discography → app lists songs → pulls metadata + genre tags → review/edit tags → process → perfect ALAC files ready for iTunes"_

---

## 🎯 **AI ASSISTANT GUIDANCE**

### **Communication Style Notes:**

- **Movie/cultural references** are used for concept explanation, NOT UI decisions
- **Example**: "Make It So" explains execution concept, actual button should be "GO" or "Start"
- **Example**: "Pre-flight" explains preparation phase, not literal aviation UI
- **Focus on intent** behind references, not literal implementation

### **When Suggesting Features:**

1. **Ask**: Does this solve a real workflow pain point?
2. **Validate**: Is this used in the actual 93% stereo or 7% multichannel workflows?
3. **Check**: Does this add complexity without proportional value?
4. **Confirm**: Is this compatible with single-user, power-user context?

### **When Discussing Architecture:**

1. **Remember**: Memory mapping is for workflow efficiency, not memory efficiency
2. **Context**: High-end hardware assumption (16GB+ RAM)
3. **Scope**: Desktop application, not web app
4. **Integration**: File compatibility, not app integration

### **When Planning Development:**

1. **Priority**: "GO" button implementation is current critical path
2. **Testing**: Focus on real audio files from user's collection
3. **Validation**: PowerShell script replacement is success metric
4. **Scope**: Tool consolidation, not feature expansion

**Remember: This is a personal productivity tool for a specific workflow, not a general-purpose audio application.**

---

## 📋 **SESSION CLOSE CHECKLIST**

### 🔄 **Before Ending Any Development Session:**

**Always update these documents to maintain consistency:**

1. **PROJECT_PLAN.md**

   - ✅ Update phase completion status (✅/⏳/🎯)
   - ✅ Add new technical decisions or architecture changes
   - ✅ Update "CURRENT STATUS" section with latest progress
   - ✅ Document any new dependencies or technical requirements

2. **AI_CONTEXT.md**

   - ✅ Add any new scope restrictions or rejected approaches
   - ✅ Update current priority section if focus changes
   - ✅ Document new technical insights or architectural decisions
   - ✅ Add any new "DO NOT SUGGEST" items discovered

3. **VALUE_PROPOSITION.md** (if messaging changes)

   - ✅ Update feature descriptions if scope changes
   - ✅ Refine user benefits based on development learnings
   - ✅ Adjust competitive positioning if new alternatives found

4. **DOCUMENT_CONSISTENCY.md**
   - ✅ Add summary of changes made in session
   - ✅ Note any terminology updates or new standards
   - ✅ Update document status and last modified dates

### ⚠️ **Critical Consistency Rules:**

- **Terminology**: Always use "volume balancing" not "loudness normalization"
- **Scope**: Single expert UI mode, no plugins, no profiles
- **Status**: Keep phase completion status accurate across all docs
- **Naming**: Document any discussions about RangeForge alternatives

### 🎯 **Session Success Criteria:**

- [ ] All changed documents use consistent terminology
- [ ] Current project status accurately reflected everywhere
- [ ] New technical decisions documented for future reference
- [ ] AI_CONTEXT.md updated with any new scope restrictions
- [ ] Future AI sessions can continue seamlessly from this point

### 📝 **Quick Update Template:**

```markdown
## Session Update: [Date]

**Changes Made:** [Brief summary]
**Documents Updated:** [List]
**Current Status:** Phase X - [Priority]
**Next Session Should:** [Action items]
```

---

_Document Purpose: Rapid AI context transfer to maintain execution consistency_  
_Last Updated: November 22, 2025_
