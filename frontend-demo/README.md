# Landing Page Integration Demo

This repo is primarily a Rust CLI, so it does **not** natively support:

- React
- TypeScript
- Tailwind CSS
- `shadcn/ui`

To integrate the provided landing component without rewriting the Rust project, this demo isolates a small frontend workspace in `frontend-demo/`.

## What was added

- React + TypeScript + Vite
- Tailwind CSS
- `components/ui` structure compatible with `shadcn/ui`
- `lib/utils.ts`
- the landing hero in `components/ui/hero-section-1.tsx`
- `animated-group.tsx` and `text-effect.tsx`
- `sheet.tsx`, `button.tsx`, `input.tsx`, and `label.tsx`

## Default paths in this demo

- Components: `frontend-demo/components/ui`
- Styles: `frontend-demo/src/styles/globals.css`

Using `components/ui` matters because that is the convention `shadcn/ui` uses for reusable primitives. It keeps shared UI separate from route-level code, avoids ad hoc component sprawl, and makes imports like `@/components/ui/button` predictable across the app. For a focused landing app, keeping the hero in the same component layer also avoids alias drift while the page is still small.

## Install

```bash
cd frontend-demo
npm install
npm run dev
```

## If you want to initialize this from the `shadcn` CLI instead

From a real frontend app root, the typical flow is:

```bash
npm create vite@latest my-app -- --template react-ts
cd my-app
npm install
npm install -D tailwindcss postcss autoprefixer
npx tailwindcss init -p
npx shadcn@latest init
```

Then make sure:

- your component alias resolves `@/*`
- your shared UI lives in `components/ui`
- your global Tailwind stylesheet is registered before rendering the app
- `framer-motion` is installed for animated landing sections
