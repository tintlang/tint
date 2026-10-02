# Accessibility

**Roles and labels.** `role||"dialog"` and `aria_*||` attributes set the HTML attributes (`aria_label`
is `aria-label`). A value is a string, `{expression}`, or `true`/`false` (an `aria_*` boolean is written as the text `true` or `false`):

```tn
Button { click||toggle aria_expanded||{open} aria_controls||"menu" "Menu" }
Label { role||"status" aria_live||"polite" "{message}" }
Icon { aria_hidden||true }
```

Supported: `role`, `aria_label`, `aria_labelledby`, `aria_describedby`, `aria_hidden`,
`aria_expanded`, `aria_selected`, `aria_checked`, `aria_pressed`, `aria_current`, `aria_live`,
`aria_controls`, `aria_modal`, `aria_busy`, `aria_disabled`, `aria_haspopup`, `aria_valuenow`,
`aria_valuemin`, `aria_valuemax`, `aria_invalid`, `aria_required`, `aria_orientation`, and
`tabindex`, `title`, `alt`.

**Keyboard.** A node with `click||` is a real `<button>`, so it takes focus, Tab order and
Enter/Space for free. Focus order is the order of the nodes. Keyboard focus shows a 2px outline
(`:focus-visible`); override it with your own style. Dialogs: see [Overlays](overlays.md).

**Reduced motion.** When the system asks for less motion (`prefers-reduced-motion: reduce`), `enter` and
`exit` keep their fades but lose their movement (`translate`, `scale`, `rotate`), and `layout` glides are
skipped. Other transitions and `view` reveals run as written. There is nothing to write.
