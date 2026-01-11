# Responsiveness Guidance

This document describes the responsive design patterns used in the Hangul Learn app.

## Overview

The app uses **Tailwind CSS v4** for responsive design with a mobile-first approach. All pages are designed to work well on:
- Mobile phones (< 640px)
- Tablets (640px - 1024px)
- Desktop (> 1024px)

**Key features:**
- Mobile-first breakpoint system
- Dark mode support via `dark:` variant
- Accessibility features (focus rings, screen reader text, touch targets)

## Tailwind Breakpoints

| Prefix | Min Width | Target Devices | Usage in Codebase |
|--------|-----------|----------------|-------------------|
| (none) | 0px | Mobile (default) | Base styles |
| `sm:` | 640px | Large phones, small tablets | 47 occurrences |
| `md:` | 768px | Tablets | 21 occurrences |
| `lg:` | 1024px | Desktop | 62 occurrences |
| `xl:` | 1280px | Large desktop | Rarely used |

**Important:** Don't think of `sm:` as "on small screens"—think of it as "at the small breakpoint and above."

## Key Patterns

### 1. Mobile Navigation (Hamburger Menu)

**Location**: `templates/base.html`

The navigation uses a hamburger menu on mobile that expands to horizontal links on desktop:

```html
<!-- Desktop nav (hidden on mobile) -->
<nav class="hidden md:flex items-center space-x-6">
  <a href="..." class="...">Link</a>
</nav>

<!-- Mobile menu button (hidden on desktop) -->
<div class="md:hidden">
  <button onclick="toggleMobileMenu()">
    <!-- hamburger icon -->
  </button>
</div>

<!-- Mobile menu panel (toggleable) -->
<div id="mobile-menu" class="hidden md:hidden">
  <!-- vertical nav links -->
</div>
```

**JavaScript toggle**:
```javascript
function toggleMobileMenu() {
  const menu = document.getElementById('mobile-menu');
  menu.classList.toggle('hidden');
  // swap hamburger/X icons
}
```

### 2. Responsive Typography

Use smaller text on mobile, larger on desktop:

```html
<!-- Headings -->
<h1 class="text-2xl sm:text-3xl">...</h1>

<!-- Large display text (Hangul characters) -->
<div class="text-5xl sm:text-7xl">ㄱ</div>

<!-- Body text -->
<p class="text-sm sm:text-base">...</p>
```

### 3. Responsive Spacing

Reduce padding/margins on mobile:

```html
<!-- Card padding -->
<div class="p-4 sm:p-6 lg:p-8">...</div>

<!-- Margins -->
<div class="mb-4 sm:mb-6">...</div>

<!-- Gaps -->
<div class="gap-2 sm:gap-4">...</div>
```

### 4. Responsive Grids

Use fewer columns on mobile:

```html
<!-- Stats grid -->
<div class="grid grid-cols-3 gap-2 sm:gap-4">...</div>

<!-- Library cards -->
<div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-3">
```

### 5. Hidden Elements on Mobile

Hide non-essential content on small screens:

```html
<!-- Hide mascot on small screens -->
<img class="hidden md:block w-16 lg:w-20" src="/static/mascot.png">

<!-- Hide table columns on mobile -->
<th class="hidden sm:table-cell">Hint</th>
<td class="hidden sm:table-cell">...</td>
```

### 6. Touch-Friendly Buttons

Make interactive elements large enough for touch:

```html
<button class="min-h-[3rem] sm:min-h-[4rem] py-3 px-4 touch-manipulation">
  Click me
</button>
```

Key properties:
- `min-h-[3rem]` or larger for touch targets
- `touch-manipulation` to prevent double-tap zoom delay
- `py-3 px-4` for adequate padding

### 7. Flex Layout Stacking

Stack horizontal layouts vertically on mobile:

```html
<div class="flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4">
  <span class="shrink-0">Label</span>
  <span>Content that may wrap</span>
</div>
```

### 8. Responsive Card Heights

Adjust fixed heights for different screens:

```html
<div class="h-56 sm:h-72">
  <!-- card content -->
</div>
```

### 9. Container Classes

Use max-width containers for content:

```html
<!-- Narrow content (cards, forms) -->
<div class="max-w-lg mx-auto">...</div>

<!-- Medium content (settings, progress) -->
<div class="max-w-2xl mx-auto">...</div>

<!-- Wide content (library) -->
<div class="max-w-4xl mx-auto">...</div>

<!-- Full-width with responsive padding -->
<div class="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">...</div>
```

### 10. Mobile TOC / Desktop Sidebar

**Location**: `templates/library/vocabulary.html`

Pattern for responsive navigation with mobile horizontal scroll and desktop sidebar:

```html
<!-- Mobile: Sticky horizontal scroll nav -->
<nav class="lg:hidden sticky top-0 z-40 -mx-4 px-4 bg-gray-50 dark:bg-gray-900">
  <div class="flex gap-2 overflow-x-auto py-2">
    <a href="#section1" class="shrink-0 px-3 py-1 rounded-full">Section 1</a>
    <a href="#section2" class="shrink-0 px-3 py-1 rounded-full">Section 2</a>
  </div>
</nav>

<!-- Desktop: Fixed sidebar -->
<aside class="hidden lg:block lg:w-56 shrink-0">
  <nav class="sticky top-4">
    <a href="#section1" class="block py-2">Section 1</a>
    <a href="#section2" class="block py-2">Section 2</a>
  </nav>
</aside>

<!-- Main content -->
<main class="flex-1 min-w-0">
  <!-- content -->
</main>
```

### 11. Responsive Button Text

Hide verbose text on mobile, show on larger screens:

```html
<button>
  <span class="hidden sm:inline">1: </span>Again
</button>

<!-- Or use different text -->
<span class="sm:hidden">Short</span>
<span class="hidden sm:inline">Longer Label</span>
```

### 12. Scale Transform for Mobile

Scale down elements on mobile instead of using different sizes:

```html
<!-- Mascot: smaller on mobile -->
<div class="scale-75 md:scale-100">
  <img src="/static/mascot.svg" class="w-32 h-32">
</div>
```

## Dark Mode Patterns

The app supports dark mode via Tailwind's `dark:` variant. Dark mode is toggled via a class on the `<html>` element.

### Basic Usage

```html
<div class="bg-white dark:bg-gray-800 text-gray-900 dark:text-white">
  Dark mode compatible element
</div>
```

### Common Dark Mode Pairs

| Light | Dark |
|-------|------|
| `bg-white` | `dark:bg-gray-800` |
| `bg-gray-50` | `dark:bg-gray-900` |
| `bg-gray-100` | `dark:bg-gray-700` |
| `text-gray-900` | `dark:text-white` |
| `text-gray-600` | `dark:text-gray-300` |
| `text-gray-500` | `dark:text-gray-400` |
| `border-gray-200` | `dark:border-gray-700` |

### Toggle Implementation

```javascript
function toggleDarkMode() {
  document.documentElement.classList.toggle('dark');
  localStorage.setItem('darkMode',
    document.documentElement.classList.contains('dark'));
}

// On page load
if (localStorage.getItem('darkMode') === 'true') {
  document.documentElement.classList.add('dark');
}
```

## Accessibility Patterns

### Screen Reader Text

Hide text visually but keep it accessible to screen readers:

```html
<button>
  <svg>...</svg>
  <span class="sr-only">Close menu</span>
</button>
```

### Focus States

Always provide visible focus indicators:

```html
<button class="focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2">
  Click me
</button>

<!-- With dark mode support -->
<a class="focus:ring-2 focus:ring-indigo-500 dark:focus:ring-offset-gray-800">
  Link
</a>
```

### Touch Target Sizes (WCAG 2.2)

Per [WCAG 2.5.8](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum):

| Level | Minimum Size | Notes |
|-------|--------------|-------|
| **AA** | 24×24 CSS pixels | Required minimum |
| **AAA** | 44×44 CSS pixels | Recommended |

**Platform guidelines:**
- iOS: 44×44 points minimum
- Android: 48×48 dp minimum

**Implementation:**
```html
<!-- Good: 48px height (py-3 = 12px * 2 + ~24px content) -->
<button class="py-3 px-4 min-h-[3rem]">Button</button>

<!-- Icon button: explicit size -->
<button class="w-11 h-11 flex items-center justify-center">
  <svg class="w-5 h-5">...</svg>
</button>
```

**Exceptions** (per WCAG):
- Inline links within text
- Browser-controlled elements (native checkboxes, radio buttons)
- Elements with equivalent accessible alternatives

### Reduced Motion

Respect user preference for reduced motion:

```html
<!-- Disable animation for users who prefer reduced motion -->
<div class="motion-safe:animate-bounce motion-reduce:animate-none">
  Animated element
</div>
```

## Animation Patterns

Custom animations defined in `src/input.css`:

| Class | Duration | Description |
|-------|----------|-------------|
| `.card-flip` | 0.6s | 3D card flip with perspective |
| `.haetae-float` | 3s | Mascot gentle floating |
| `.haetae-blink` | 4s | Mascot eye blinking |
| `.haetae-tail-wag` | 0.3s | Mascot tail wagging |
| `.firework-burst` | 0.8s | Confetti celebration |
| `.fade-in` | 0.3s | Toast/notification entrance |

**Example usage:**
```html
<div class="haetae-float">
  <img src="/static/mascot.svg" alt="Haetae">
</div>
```

## File-Specific Patterns

### base.html
- Logo: SVG icon + full text on desktop, shortened on mobile (`hidden sm:inline` / `sm:hidden`)
- Hamburger menu: `md:hidden` / `hidden md:flex`
- Floating mascot: `hidden sm:block` (hidden on mobile, visible on larger screens)
- Container: `px-4 sm:px-6 lg:px-8`

### index.html (Home Page)
- Dedicated mascot: Always visible, centered above title
- Title: `text-2xl sm:text-3xl`
- Subtitle: `text-sm sm:text-base`

### interactive_card.html
- Character display: `text-5xl sm:text-7xl`
- Choice buttons: `min-h-[4rem] sm:min-h-[5rem]`
- Button grid: `grid grid-cols-2 gap-2 sm:gap-3`

### progress.html
- Stats grid: `grid-cols-3 gap-2 sm:gap-4`
- Stat values: `text-2xl sm:text-3xl`
- Card padding: `p-3 sm:p-4`

### practice_card.html
- Card height: `h-56 sm:h-72`
- Front text: `text-5xl sm:text-7xl`
- Back text: `text-3xl sm:text-4xl`

### reference/tier*.html
- Table columns: Hide `Hint` column on mobile with `hidden sm:table-cell`
- Letter size: `text-3xl sm:text-4xl`
- Sound text: `text-sm sm:text-base`

### guide.html
- Score labels: Stack vertically on mobile with `flex-col sm:flex-row`
- Hint labels: Stack vertically on mobile

### library/*.html
- Card grid: `grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5`
- Mobile TOC: `lg:hidden sticky top-0` with `overflow-x-auto`
- Desktop sidebar: `hidden lg:block lg:w-56`

## Testing Checklist

When adding new features, verify:

1. [ ] Navigation menu works on mobile (hamburger opens/closes)
2. [ ] Text is readable without zooming
3. [ ] Buttons are large enough to tap (min 44×44px touch target for AAA, 24×24px for AA)
4. [ ] Forms are usable (inputs not too small)
5. [ ] Tables don't break layout (use `overflow-x-auto` or hide columns)
6. [ ] Images scale appropriately
7. [ ] No horizontal scroll on main content
8. [ ] Modal/overlay dialogs are positioned correctly
9. [ ] Dark mode colors have sufficient contrast
10. [ ] Focus states are visible
11. [ ] Screen reader text is provided for icon-only buttons
12. [ ] Animations respect `prefers-reduced-motion`

## Browser DevTools

Test responsiveness using browser developer tools:

1. Open DevTools (F12 or Cmd+Option+I)
2. Click the device toolbar icon (or Cmd+Shift+M)
3. Select device presets or enter custom dimensions
4. Test at these common widths:
   - 320px (iPhone SE, minimum supported)
   - 375px (iPhone mini)
   - 414px (iPhone Plus/Max)
   - 768px (iPad portrait)
   - 1024px (iPad landscape / small laptop)
   - 1440px (Desktop)

## Common Gotchas

### Fixed Widths
Avoid fixed pixel widths that break on mobile:
```html
<!-- Bad -->
<div class="w-96">...</div>

<!-- Good -->
<div class="w-full max-w-md">...</div>
```

### Long Text
Handle long text that might overflow:
```html
<!-- Truncate -->
<span class="truncate">Long text here</span>

<!-- Line clamp -->
<p class="line-clamp-2">Multi-line text...</p>

<!-- Word break -->
<span class="break-words">longunbrokenword</span>
```

### Flex Shrink
Prevent labels from shrinking in flex containers:
```html
<div class="flex">
  <span class="shrink-0">Label:</span>
  <span class="min-w-0">Content</span>
</div>
```

### Table Overflow
Always wrap tables for horizontal scroll fallback:
```html
<div class="overflow-x-auto">
  <table class="w-full">...</table>
</div>
```

### Reduced Motion
Always provide `motion-reduce:` alternatives for animations:
```html
<!-- Bad: Animation with no fallback -->
<div class="animate-bounce">...</div>

<!-- Good: Respects user preference -->
<div class="motion-safe:animate-bounce motion-reduce:animate-none">...</div>
```

### Dark Mode Focus Rings
Adjust focus ring offset for dark backgrounds:
```html
<button class="focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 dark:focus:ring-offset-gray-800">
```

## Known Issues

The following issues have been identified but not yet fixed:

| Priority | Issue | File | Description | Fix |
|----------|-------|------|-------------|-----|
| **P1** | Small touch targets | Various | Some `p-2` icon buttons are ~32px, below WCAG 44px | Add `min-w-11 min-h-11` to icon buttons |
| **P1** | Mobile menu fixed width | `base.html` | `w-48` could overflow on phones <384px | Change to `w-full max-w-xs` |
| **P2** | Large typography overflow | `interactive_card.html`, `practice_card.html` | `text-7xl` may overflow narrow containers | Add `overflow-hidden` wrapper |
| **P3** | TOC sidebar width | `library/vocabulary.html` | `lg:w-56` leaves only 800px at 1024px | Use `lg:w-48 xl:w-56` |
| **P3** | Custom calc widths | `settings.html` | `w-[calc(50%-0.375rem)]` not responsive | Refactor to grid layout |

**Priority levels:**
- **P1 (High)**: Accessibility/WCAG compliance or breaks on common devices
- **P2 (Medium)**: Usability issues affecting some users
- **P3 (Low)**: Edge cases or minor visual issues

## Resources

- [Tailwind CSS Responsive Design](https://tailwindcss.com/docs/responsive-design)
- [WCAG 2.5.8 Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum)
- [Accessible Touch Target Sizes](https://www.smashingmagazine.com/2023/04/accessible-tap-target-sizes-rage-taps-clicks/)
