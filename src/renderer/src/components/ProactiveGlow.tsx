import React from 'react'

export default function ProactiveGlow() {
  return (
    <>
      <style>{`
        .vision-glow {
            position: absolute;
            inset: 0;
            pointer-events: none;
            box-shadow: inset 0 0 70px rgba(139, 92, 246, 0.2);
            opacity: 0.7;
            z-index: 100;
        }
      `}</style>
      <div className="vision-glow" />
    </>
  )
}
