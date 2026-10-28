declare module 'react-dom' {
  export function createPortal(children: any, container: any, key?: any): any;
  export function flushSync<R>(fn: () => R): R;
}

declare module 'react-dom/server' {
  export function renderToStaticMarkup(element: any): string;
  export function renderToString(element: any): string;
}
