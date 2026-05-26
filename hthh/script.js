document.addEventListener('DOMContentLoaded', () => {
    const btn = document.getElementById('celebrateBtn');

    // ฟังก์ชันยิงพลุ
    const fireConfetti = () => {
        const duration = 3 * 1000;
        const end = Date.now() + duration;

        (function frame() {
            confetti({
                particleCount: 3,
                angle: 60,
                spread: 55,
                origin: { x: 0 },
                colors: ['#ff0000', '#ffa500', '#ffff00']
            });
            confetti({
                particleCount: 3,
                angle: 120,
                spread: 55,
                origin: { x: 1 },
                colors: ['#0000ff', '#00ff00', '#ff00ff']
            });

            if (Date.now() < end) {
                requestAnimationFrame(frame);
            }
        }());
    };

    // ยิงพลุตอนโหลดหน้าเว็บเสร็จ
    fireConfetti();

    // ยิงพลุเมื่อกดปุ่ม
    btn.addEventListener('click', () => {
        fireConfetti();
        
        // เพิ่มแอนิเมชันให้ปุ่มเล็กน้อย
        btn.style.transform = 'scale(0.9)';
        setTimeout(() => {
            btn.style.transform = 'scale(1)';
        }, 100);
    });
});