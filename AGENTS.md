# Capty - Screenshot Tool (Electron + React + Bun)

## Supported Platforms

- MacOS 15 or later (Intel + Apple Silicon) — full feature set
- Windows 10/11 (x64) — source-build support for full-screen, area, and window screenshots; editor, history, pin, and cloud upload; OCR; QR scanning; desktop icons and wallpaper; timer capture; display and window selectors; freeze screen; scroll capture; all-in-one; print; recording; video editor; and transcription. The screenshot capture sound remains macOS-only. No public Windows release is available yet.

Windows native functionality is provided by the Rust daemon in `src/main/daemon-win/`, which speaks the same JSON-RPC protocol as the macOS Swift daemon. Windows screen pixels come from the daemon's `screenshot` module, which captures with Windows Graphics Capture and, on HDR displays, reads the scRGB float surface and normalizes it by the OS SDR white level before encoding sRGB. Electron `desktopCapturer` is not used for screenshots because it hands back frames already clipped to 8-bit, which is washed out on HDR. The area overlay stays an Electron window (`src/main/capture/area-overlay/`): the daemon writes each display's frozen frame to a temp `.bmp` and retains it in memory, the overlay renders that file, and the confirmed selection is cropped from the retained frame and released. The `screenshot` module is intentionally Windows-only — macOS captures with the `screencapture` CLI — and is the one documented exception to the cross-platform module parity rule below. Gate platform-specific surfaces with `isFeatureSupported()` from `src/main/system/capabilities.ts` or `src/renderer/utils/capabilities.ts`; feature support is defined in `src/types/capabilities.ts`. Build the daemon with `bun run build-daemon-win` or the complete Windows package with `bun run build-win`.

## Tests

Write tests for new features and bug fixes according to the following configuration:

- **Main**: Vitest - Use `bun run test`
- **Renderer**: Not implemented yet
- **Coverage**: `bun run test:coverage` - reports in `coverage/` folder

Tests use Vitest with vi.mock() for mocking modules (electron, AWS SDK, config), class-based mocks for constructors, dynamic imports (await import()) after vi.resetModules() to get fresh module instances with updated mocks, and are organized in src/main/**tests**/unit/ or src/main/**tests**/integration/ - run with bun run test

## Code Style

- **Formatting**: Use `bun run format` to fix, `bun run format:check` to verify
- **Linting**: ESLint - Use `bun lint` to make sure no lint errors
- **All checks**: `bun run checks` runs typecheck + lint + format check + tests (verify only, no fixing)
- **Imports**: Group by external → components → hooks → types → utils. Use `type` for type-only imports (`import type { ToolType }`)
- **Types**: Store shared types in `src/types/` (accessible to main + renderer). Use discriminated unions for polymorphic data
- **Naming**: kebab-case (components), camelCase (functions/vars), SCREAMING_SNAKE_CASE (constants like `MACOS_COLORS`)
- **Components**: Export as default, define interfaces inline. Prefer small, reusable components over monolithic files
- **React**: Use functional components, hooks (`useCallback` for event handlers), avoid unnecessary re-renders
- **Icons**: Use Lucide React (`lucide-react`)
- **Styling**: Tailwind classes only - prefer built-in values (e.g., `gap-1`) over arbitrary values (e.g., `px-[20px]`)
- **Error Handling**: Check for null/undefined (e.g., `screenshotWindow?.method()`) and handle errors gracefully

## Architecture

- **Electron**: Main process (`src/main/`), renderer (`src/renderer/`), preload (`src/preload/`)
- **IPC**: Use `ipcMain.on` (main) / `ipcRenderer.send` (renderer) for process communication
- **State**: Local hooks (`useState`, custom hooks in `src/renderer/hooks/`) - no global state library

## Performance & Design

- Minimize bundle size, memory, and CPU usage. Use lazy loading where appropriate
- Follow Shadcn design guidelines
- Use Shadcn components unless a custom component is necessary
- For frameless Electron windows with solid backgrounds, use transparent: false with a matching backgroundColor instead of transparent: true to avoid dark border artifacts on macOS.

## Top Level Rules

- Security first
- Maintainability
- Scalability
- Clean Code
- Clean Architecture
- Best Practices
- No Hacky Solutions

## General Guidelines

- Don't implement hacky solutions to just make it work. We need proper solutions.
- Never build or run dev. user will do.
- Write less code and maintainable code
- Always put modularity and reusability in priority
- Prefer tailwind's built-in classes over custom sizes like px[20px]
- Try to create re-usable components instead of writing big chunks of code
- Be mindful about app size and performance and memory and cpu usage.
- Use types and interfaces and store them in the src/types folder so they can be used in both main and renderer processes.
- Learn from project's structure and implement new features in the same way.
- Break code into smaller components and files.
- When implementing a feature that can also be configurable, ask user's opinion to make it configurable in the app settings or not.
- Consider SOLID principles and best practices while writing code.
- If there is a refactor needed, ask user's opinion first.
- Avoid creating big files and components. Instead, modularize and break them into smaller pieces.
- NEVER NEVER NEVER code comment!
- Native functionality is provided by a unified daemon (`capty-daemon`): Swift on macOS (`src/main/daemon/`, build with `./scripts/build-daemon.sh` for universal arm64 + x86_64) and Rust on Windows (`src/main/daemon-win/`, build with `bun run build-daemon-win`). Both speak the same JSON-RPC protocol over stdin/stdout, and module contracts must stay identical across platforms.
- The daemon uses JSON-RPC over stdin/stdout.
- When adding new native modules, add them to `src/main/daemon/Modules/` and register in `main.swift` (macOS), and to `src/main/daemon-win/src/modules/` and register in `main.rs` (Windows).
- When adding assets to the project like images, icons, sounds, etc, make sure you also consider them for production build and packing and notarizing to work on packaged app too.
- Don't patch symptoms! Fix the root cause of the issues.
- Use early returns to reduce nesting
- Important to avoid nested if-else statements
- Prefer switch-case for multiple conditions
- Use agnostic implementations everywhere possible so we can re-use those codes
- Don't use just delays instead of promises! Handle them properly. (using delay is basically hacking and we should avoid)
- Code duplications should be avoided at any cost.
- Don't install third-party packages without approval.
- The result shown in the video-editor must be identical to the results after export. This applies to zoom, trims, cuts and other layers applied to the video.
- We might have unused codes that seemed to be in-use! if you face any of them, make sure if they are used or not then decide to keep or remove them.
- For menu icons, Use the png icon and user will download them.
- No guessing and No assumption! Work with certainity.

## Coding examples

### Nesting

Bad code example:

```typescript
if (
  selection.x !== undefined &&
  selection.y !== undefined &&
  selection.width !== undefined &&
  selection.height !== undefined
) {
  if (selection.status === 'selected') {
    showAllInOneControl({
      x: selection.x,
      y: selection.y,
      width: selection.width,
      height: selection.height,
    });
  } else if (selection.status === 'updated') {
    updateAllInOnePosition({
      x: selection.x,
      y: selection.y,
      width: selection.width,
      height: selection.height,
    });
  }
}
```

Good code example:

```typescript
if (
  selection.x === undefined ||
  selection.y === undefined ||
  selection.width === undefined ||
  selection.height === undefined
) {
  return;
}

const bounds = {
  x: selection.x,
  y: selection.y,
  width: selection.width,
  height: selection.height,
};

if (selection.status === 'selected') {
  showAllInOneControl(bounds);
  return;
}

if (selection.status === 'updated') {
  updateAllInOnePosition(bounds);
}
```

### IPC Naming

Bad example: `'screenshot-take'`, `'screenshot-save-as'`, `screenshot:saveAs`

Good example: `'screenshot:take'`, `'screenshot:save-as'`, `screenshot:pin:toggle`

### Tailwind classes

Bad example: `p-[20p]x]`, `text-[14px]`, `bg-red-500`

Good example: `p-5`, `text-sm`, `bg-destructive`

### Wrapper functions

Avoid creating wrapper functions that only repeats the job

Bad example:

```typescript
function getAreaSelectorBinaryPath(): string {
  return getNativeBinaryPath('area-selector');
}
```

Good example:

Just use `getNativeBinaryPath` without wrapper.

## Questions

- If you have questions, Use the question tool only for asking questions.

## Code Review

- Review the PRs/Code against the purpose of the PR/Issue/Asked. If you find unrelated issues to the PR during the review, Report them in a separate section.
- Apply review recommendations only after user's confirmation.

## PR and Commits

- After end of each task that has a chance to create a PR for it, suggest PR title and description
- Add feat: or fix: or internal: prefixes to the PRs or Commits because thats how they will be appeared on release logs.
