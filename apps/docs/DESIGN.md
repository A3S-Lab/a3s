---
name: A3S Ecosystem Site
description: A bilingual project directory built around verified ownership, delivery state, public sites, and source.
colors:
  background: "#ffffff"
  paper: "#f3f5f8"
  paper-blue: "#f1f6ff"
  panel: "#ffffff"
  panel-strong: "#0e1b35"
  line: "#dce4f0"
  line-strong: "#bfd0e8"
  text: "#101827"
  muted: "#56657b"
  faint: "#8490a3"
  blue: "#1264ff"
  blue-deep: "#084ed0"
  blue-soft: "#eaf2ff"
  green: "#0c9b70"
  green-soft: "#eaf8f3"
typography:
  sans: "'Geist Variable', 'PingFang SC', 'Microsoft YaHei', ui-sans-serif, system-ui, sans-serif"
  mono: "'Geist Mono Variable', 'SFMono-Regular', 'Cascadia Code', Consolas, monospace"
layout:
  max-width: "1440px"
  navigation-height: "72px"
  mobile-breakpoint: "760px"
---

# Design System: A3S Ecosystem Site

## Purpose

The site is an index for the A3S project family. It helps visitors identify the
project that owns a capability, check its delivery stage, inspect the real
product site, and continue to source or installation instructions. It does not
embed product-specific interfaces.

The visual language should feel like a maintained engineering catalog. Large
editorial type introduces the system, while borders, release metadata, and
committed site previews provide evidence. Decorative effects stay behind the
content and never compete with project names, status, or destinations.

## Color

- White is the dominant page and card surface.
- Cool gray-blue paper separates sections and preview chrome.
- `#1264ff` marks links, primary actions, focus, and active controls.
- `#0c9b70` is reserved for verified public delivery state.
- Amber and violet may classify secondary metadata, but they must not imply
  progress or health.
- Dark navy belongs to install terminals and other literal code surfaces.

Delivery stages are categories. Never turn them into percentages, progress
bars, or decorative meters.

## Typography

Use Geist Variable for interface and editorial text. Chinese uses PingFang SC
and Microsoft YaHei fallbacks. Use Geist Mono only for versions, channels,
commands, compact labels, and other machine-facing values.

- Hero statements use a tight line height and restrained line length.
- Section headings use clear scale changes instead of extra decoration.
- Project names remain visually stronger than status, release, and repository
  metadata.
- Body copy should be direct, specific, and short enough to scan once.

## Layout

Content is centered within a 1440px frame. The homepage moves through one
system story in this order: global workspace, ecosystem directory, operating
principles, local quick start, and source call to action.

The public-site showcase uses an asymmetric 12-column grid. Each card includes
a committed screenshot, checked delivery label, plain-language responsibility,
and separate site and source destinations. The 34-project directory below it
supports search and layer filters without hiding ownership or release data.

Desktop navigation remains 72px high. It contains ecosystem sections, Desktop
downloads, locale selection, and source access. Individual products are reached
through the directory and footer, not top-level product menus.

On small screens, navigation collapses into one disclosure, showcase cards
become a single column, and controls retain at least a 44px touch target.

## Components

### Project preview card

Use a 16px corner, a one-pixel cool border, and a shallow resting shadow. The
browser-style preview frame is functional context for a real screenshot. Hover
may lift the card by up to four pixels, but the destination must remain clear
without motion.

### Directory card

Keep the card compact and text-led. Project name, ownership statement, delivery
stage, current version or channel, public site, and source are distinct fields.
Filtering must not change or synthesize status.

### Navigation

Keep the site mark, section links, locale control, and GitHub control visually
separate. Product links do not belong in the primary navigation. Keyboard focus
uses a visible cobalt outline and returns correctly when the mobile disclosure
closes.

### Install terminal

The terminal is a literal command surface with selectable platform tabs and a
copy action. It uses the dark navy surface, compact mono labels, and exact
commands from the repository. It must not imitate an animated agent session.

## Motion and depth

Permanent structure uses borders first. Shadows identify sticky navigation,
preview cards, and the install terminal. Motion is limited to small hover lifts,
icon movement, and disclosure transitions, and is disabled when reduced motion
is requested.

## Accessibility

- Provide a skip link and visible focus for every interactive element.
- Preserve semantic headings and landmark labels in both languages.
- Do not encode delivery state by color alone.
- Keep preview alt text localized and describe the product, not the screenshot
  decoration.
- Maintain Chinese and English route and control parity.

## Boundaries

- Do keep product previews tied to committed captures of public sites.
- Do show checked releases and categorical delivery stages.
- Do route detailed product work to the repository that owns it.
- Do not add standalone product editors or playground routes.
- Do not add individual product names to the primary navigation.
- Do not use glass cards, ornamental dashboards, fake metrics, or generated
  interface screenshots as evidence.
