# Responsiveness Guidance

This document describes the responsive design patterns used in the Hangul Learn app.

## Overview

The app uses Tailwind CSS for responsive design with a mobile-first approach. All pages are designed to work well on:
- Mobile phones (< 640px)
- Tablets (640px - 1024px)
- Desktop (> 1024px)

## Tailwind Breakpoints

| Prefix | Min Width | Target Devices |
|--------|-----------|----------------|
| (none) | 0px | Mobile (default) |
| `sm:` | 640px | Large phones, small tablets |
| `md:` | 768px | Tablets |
| `lg:` | 1024px | Desktop |
| `xl:` | 1280px | Large desktop |

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

### library.html
- Card grid: `grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5`

## Testing Checklist

When adding new features, verify:

1. [ ] Navigation menu works on mobile (hamburger opens/closes)
2. [ ] Text is readable without zooming
3. [ ] Buttons are large enough to tap (min 44x44px touch target)
4. [ ] Forms are usable (inputs not too small)
5. [ ] Tables don't break layout (use `overflow-x-auto` or hide columns)
6. [ ] Images scale appropriately
7. [ ] No horizontal scroll on main content
8. [ ] Modal/overlay dialogs are positioned correctly

## Browser DevTools

Test responsiveness using browser developer tools:

1. Open DevTools (F12 or Cmd+Option+I)
2. Click the device toolbar icon (or Cmd+Shift+M)
3. Select device presets or enter custom dimensions
4. Test at these common widths:
   - 375px (iPhone SE/mini)
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
