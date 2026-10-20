declare module 'react-dom/server' {
  export function renderToStaticMarkup(element: any): string;
  export function renderToString(element: any): string;
}
