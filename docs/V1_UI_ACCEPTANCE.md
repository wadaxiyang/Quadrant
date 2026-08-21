# V1 UI Acceptance

Stage 16 accessibility, DPI, keyboard, and visual-polish acceptance record.

| Area | Result | Evidence / limitation |
|---|---|---|
| 100% DPI layout | Not tested | Requires interactive Windows desktop session. XAML uses content-driven rows and shared spacing resources. |
| 125% DPI layout | Not tested | Requires interactive Windows desktop session. |
| 150% DPI layout | Not tested | Requires interactive Windows desktop session. |
| 200% DPI layout | Not tested | Requires interactive Windows desktop session. |
| 920x620 minimum main window | Not tested | Requires interactive Windows GUI. Main window retains the specified minimum size and scrollable quadrant lists. |
| Task editor stays within work area | Pass | Editor caps `MaxWidth`/`MaxHeight` to `SystemParameters.WorkArea` and uses a vertical `ScrollViewer`. |
| DatePicker/ComboBox do not clip | Not tested | Requires interactive Windows GUI at high DPI; form content is scrollable. |
| Tab order | Pass | Uses normal WPF logical XAML order; no custom focus visual suppression or `TabIndex` overrides. |
| Enter/Esc | Pass | Default/cancel buttons remain on editor dialogs; Quick Add handles Enter/Esc through WPF dialog behavior. |
| Ctrl+F | Pass | Main window focuses and selects the search box. |
| Ctrl+1..4 Quick Add | Pass | Supports D1-D4 and NumPad1-NumPad4. |
| Space/Enter task completion | Pass | Focused task card completes on Space/Enter; focused action buttons retain their own command behavior. |
| Focus visual | Pass | No focus visual styles are removed or replaced. |
| Accessibility names | Pass | Task actions, filters, search, Quick Add quadrant buttons, and dialog actions expose names/tooltips. |
| Task card semantic text | Pass | UI Automation name combines title, due text, and due status text. |
| High Contrast | Not tested | Requires interactive Windows High Contrast session. Text/backgrounds use Fluent/system resources; overdue also has text. |
| Magic numbers / repeated styles | Pass | Stage 16 changes use existing resource tokens; toolbar height is content-driven and UI strings are centralized. |
| Gradients / shadows / emoji | Pass | No gradients, batch shadows, or emoji UI icons found in `src/Quadrant.App`. |

