import React from 'react'

export default function ProactiveGlow() {
  return (
    <>
      <style>{`
        .vision-glow {
            position: absolute;
            inset: 0;
            pointer-events: none;
            box-shadow: inset 0 0 120px rgba(139, 92, 246, 0.35);
            z-index: 100;
            animation: vision-pulse-glow 3s ease-in-out infinite alternate;
        }

        @keyframes vision-pulse-glow {
            from { 
                box-shadow: inset 0 0 60px rgba(139, 92, 246, 0.15); 
                opacity: 0.4;
            }
            to { 
                box-shadow: inset 0 0 180px rgba(139, 92, 246, 0.45); 
                opacity: 1;
            }
        }
      `}</style>
      <div className="vision-glow" />
    </>
  )
}
