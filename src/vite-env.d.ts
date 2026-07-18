/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<Record<string, never>, Record<string, never>, unknown>;
  export default component;
}

declare module "@chenglou/pretext" {
  export function prepare(text: string, font: string): Promise<unknown>;
  export function layout(prepared: unknown, width: number): { height: number; lineCount: number };
}
