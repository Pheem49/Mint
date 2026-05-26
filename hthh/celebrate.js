document.addEventListener('DOMContentLoaded', () => {
    // 1. Fire confetti as soon as the page loads
    confetti({
        particleCount: 150,
        spread: 70,
        origin: { y: 0.6 },
        colors: ['#f8b500', '#e67e22', '#d35400', '#ffffff']
    });

    const magicBtn = document.getElementById('magicButton');

    // 2. Fire random confetti when the button is clicked
    magicBtn.addEventListener('click', () => {
        const duration = 3 * 1000;
        const end = Date.now() + duration;

        (function frame() {
            confetti({
                particleCount: 5,
                angle: 60,
                spread: 55,
                origin: { x: 0 },
                colors: ['#f8b500', '#e67e22']
            });
            confetti({
                particleCount: 5,
                angle: 120,
                spread: 55,
                origin: { x: 1 },
                colors: ['#d35400', '#ffffff']
            });

            if (Date.now() < end) {
                requestAnimationFrame(frame);
            }
        }());
    });
});