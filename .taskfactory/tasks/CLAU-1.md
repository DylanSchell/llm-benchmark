# Task CLAU-1: Fix Exercise Selection List Rendering

## Acceptance Criteria

1. All exercise checkboxes within each language's exercise list must align vertically in a clean column, regardless of exercise name length
2. A 'Deselect all' button or link must be added alongside the existing 'Select all' checkbox for each language
3. The exercise selection lists should have consistent spacing and alignment across all languages
4. Both 'Select all' and 'Deselect all' actions must correctly update the internal state and UI reflection
5. The improved layout should remain functional on different screen sizes

## Plan

### Goal
Fix exercise selection list rendering in run.html to improve checkbox alignment and add 'Deselect all' functionality.

### Steps

1. **Improve CSS for exercise items** - Add fixed checkbox width and proper flexbox layout to prevent text wrapping from affecting checkbox position

2. **Add 'Deselect all' button** - Create a dedicated button alongside 'Select all' with corresponding JavaScript handler to clear all selections

3. **Refactor updateExerciseList()** - Update the rendering function to use improved HTML structure with aligned checkboxes

4. **Test across all languages** - Verify alignment and functionality for Java, Go, JavaScript, Python, Rust, and C++

### Validation

- [ ] Checkboxes in exercise lists are vertically aligned for all languages
- [ ] 'Select all' checkbox/button works correctly
- [ ] 'Deselect all' button appears and functions correctly
- [ ] Exercise names with varying lengths don't break the layout
- [ ] Internal state (selectedExercises) is updated correctly by both actions

### Cleanup

None required.

## Research Notes

**File:** `/Users/dylan/Developer/claude-benchmark/src/main/resources/templates/run.html`

**Current Issues:**
- Exercise checkboxes are wrapped in div elements with nested labels causing misalignment
- Only 'Select all' functionality exists; no 'Deselect all' option
- CSS uses simple flex layout without fixed widths for checkbox column

**Solution Approach:**
- Add CSS rule for `.exercise-item input[type="checkbox"]` with fixed width/margin
- Use flexbox properly to keep checkboxes in aligned column
- Replace single checkbox with two buttons: "Select All" and "Deselect All"
- Add `deselectAll(language)` JavaScript function
