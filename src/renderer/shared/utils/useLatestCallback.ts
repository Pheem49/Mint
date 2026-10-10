import { useCallback, useLayoutEffect, useRef } from 'react'

// Memoized messages keep stable event handlers while their actions/settings
// can change. Events always call the last committed implementation.
export function useLatestCallback<T extends (...args: any[]) => any>(callback: T): T {
  const latest = useRef(callback)
  useLayoutEffect(() => { latest.current = callback }, [callback])
  return useCallback(((...args: Parameters<T>) => latest.current(...args)) as T, [])
}
