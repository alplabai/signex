/// <reference types="vite/client" />

// Optional dependency — dynamic import only, fails gracefully at runtime
declare module "three" {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const THREE: any;
  export = THREE;
}
