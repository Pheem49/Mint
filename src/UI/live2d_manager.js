/**
 * Live2DManager - Encapsulates Live2D model loading, fitting, and lip-sync logic.
 */
window.Live2DManager = {
    app: null,
    model: null,
    resizeObserver: null,
    lipSyncInterval: null,
    expIndex: 0,
    lastInteractionAt: 0,
    expressionToastTimeout: null,
    expressionParamIds: [
        'Param54',
        'Param55',
        'Param68',
        'Param76',
        'Param91',
        'Param93',
        'Param94',
        'Param96',
        'ParamAngleY',
        'ParamAngleZ',
        'ParamEyeBallX',
        'ParamEyeBallY',
        'ParamMouthForm',
        'ParamMouthOpenY'
    ],
    expressionNames: [
        { id: null, label: 'Normal' },
        { id: 'Apron', label: 'Apron' },
        { id: 'Dazed', label: 'Dazed' },
        { id: 'Photo', label: 'Photo' },
        { id: 'Glasses', label: 'Glasses' },
        { id: 'Pen', label: 'Writing' },
        { id: 'Click', label: 'Blush' },
        { id: 'CatFilter', label: 'Cat Ears' },
        { id: 'DazedEyes', label: 'Dazed Eyes' }
    ],

    async loadModel(mountEl, statusEl, shellEl) {
        this.statusEl = statusEl; // Store for later use
        if (!mountEl) return;
        if (statusEl) {
            statusEl.classList.remove('is-error');
            statusEl.innerHTML = `
                <div class="loader-dots">
                    <span></span><span></span><span></span>
                </div>
                <div style="font-size: 0.7rem; opacity: 0.8; letter-spacing: 0.05em;">SYNCHRONIZING MINT...</div>
            `;
        }
        if (!window.PIXI || !window.PIXI.live2d) {
            const message = 'Live2D runtime is not available.';
            console.error(message);
            if (statusEl) {
                statusEl.classList.add('is-error');
                statusEl.textContent = message;
            }
            return;
        }

        try {
            window.PIXI.live2d.Live2DModel.registerTicker(window.PIXI.Ticker);

            this.app = new window.PIXI.Application({
                autoDensity: true,
                antialias: true,
                backgroundAlpha: 0,
                resizeTo: mountEl,
                resolution: window.devicePixelRatio || 1
            });

            mountEl.prepend(this.app.view);

            const modelUrl = new URL('../../models/Shiroko_Model/Shiroko/Shiroko_Core/%E9%9D%A2%E9%A5%BC0.model3.json', window.location.href).href;
            this.model = await window.PIXI.live2d.Live2DModel.from(modelUrl, {
                autoInteract: true
            });
            this.expressionToastEl = document.getElementById('expression-toast');

            this.model.anchor.set(0.5, 0.5);
            this.app.stage.addChild(this.model);

            // -- Interaction Setup --
            this.model.interactive = true;
            this.model.buttonMode = true;

            // Tap Interaction. This model does not define Cubism HitAreas, so use
            // normalized model coordinates to provide stable region reactions.
            this.model.on('pointertap', (e) => this.handleModelTap(e));
            this.model.on('hit', (hitAreaNames) => {
                console.log(`[Live2D] Runtime hit detected: ${hitAreaNames}`);
            });

            const fitModel = () => {
                if (!this.model || !mountEl) return;
                const mountWidth = mountEl.clientWidth || 460;
                const mountHeight = mountEl.clientHeight || 620;
                this.app.renderer.resize(mountWidth, mountHeight);

                const internal = this.model.internalModel || {};
                const modelWidth = internal.width || internal.originalWidth || this.model.width || 1;
                const modelHeight = internal.height || internal.originalHeight || this.model.height || 1;
                const widthScale = mountWidth / Math.max(modelWidth, 1);
                const heightScale = mountHeight / Math.max(modelHeight, 1);
                
                // Reduced zoom to 2.0 as requested
                const scale = Math.min(widthScale, heightScale) * 1.8;

                this.model.scale.set(scale);
                // Adjusted Y offset to 1.0 as requested
                this.model.position.set(mountWidth / 2, mountHeight / 2 + mountHeight * 0.6);
            };

            requestAnimationFrame(() => {
                fitModel();
                requestAnimationFrame(fitModel);
            });
            this.resizeObserver = new ResizeObserver(fitModel);
            this.resizeObserver.observe(mountEl);

            shellEl?.classList.add('is-live2d-ready');
            if (statusEl) statusEl.textContent = '';
            this.model.motion('Idle', 0).catch(() => {});
        } catch (error) {
            console.error('Failed to load Live2D model:', error);
            shellEl?.classList.remove('is-live2d-ready');
            if (statusEl) {
                statusEl.classList.add('is-error');
                statusEl.textContent = `Live2D failed: ${error && error.message ? error.message : String(error)}`;
            }
        }
    },

    showStatus(text, duration = 2000) {
        if (!this.statusEl) return;
        this.statusEl.textContent = text;
        this.statusEl.style.opacity = '1';
        
        if (this.statusTimeout) clearTimeout(this.statusTimeout);
        this.statusTimeout = setTimeout(() => {
            this.statusEl.style.opacity = '0';
            setTimeout(() => {
                if (this.statusEl.style.opacity === '0') this.statusEl.textContent = '';
            }, 500);
        }, duration);
    },

    handleModelTap(event) {
        if (!this.model) return;

        const now = Date.now();
        if (now - this.lastInteractionAt < 3000) return;

        const region = this.getInteractionRegion(event);
        if (!region) return;
        this.lastInteractionAt = now;
        const expressionId = region.expression || 'Click';

        console.log(`[Live2D] Interaction: ${region.id}`, region);
        this.applyExpression(expressionId);
        this.showStatus(region.label, 1600);

        window.dispatchEvent(new CustomEvent('live2d-model-interaction', {
            detail: {
                region: region.id,
                label: region.label,
                prompt: region.prompt
            }
        }));

        setTimeout(() => {
            const currentIdx = this.expIndex === 0 ? 0 : this.expIndex;
            const prevExp = this.expressionNames[currentIdx]?.id;
            this.applyExpression(prevExp);
        }, 2000);
    },

    getInteractionRegion(event) {
        try {
            const point = this.getPointerViewportPoint(event);
            if (!point) return null;
            const { x, y } = point;

            if (this.isPointInZone(x, y, 0.37, 0.395, 0.25, 0.13)) {
                return {
                    id: 'face',
                    label: 'Cheek Poke',
                    expression: 'Click',
                    prompt: 'ผู้ใช้จิ้มแก้มโมเดลของมิ้นท์ ตอบกลับแบบสั้น ๆ เขินหรือแซวกลับเบา ๆ เป็นภาษาเดียวกับผู้ใช้'
                };
            }

            if (this.isPointInZone(x, y, 0.35, 0.30, 0.29, 0.09)) {
                return {
                    id: 'head',
                    label: 'Head Pat',
                    expression: 'Dazed',
                    prompt: 'ผู้ใช้ลูบหัวโมเดลของมิ้นท์ ตอบกลับแบบสั้น ๆ น่ารัก เขินนิด ๆ และเป็นภาษาเดียวกับผู้ใช้'
                };
            }

            const isLeftHand = this.isPointInZone(x, y, 0.17, 0.65, 0.19, 0.17);
            const isRightHand = this.isPointInZone(x, y, 0.70, 0.67, 0.17, 0.17);
            if (isLeftHand || isRightHand) {
                return {
                    id: isLeftHand ? 'left-hand' : 'right-hand',
                    label: 'Hand Tap',
                    expression: 'Pen',
                    prompt: 'ผู้ใช้แตะมือของโมเดลมิ้นท์ ตอบกลับแบบสั้น ๆ เหมือนพร้อมช่วยงานหรือรับคำสั่ง เป็นภาษาเดียวกับผู้ใช้'
                };
            }

            if (this.isPointInZone(x, y, 0.34, 0.72, 0.32, 0.20)) {
                return {
                    id: 'lower-body',
                    label: 'Careful',
                    expression: 'Photo',
                    prompt: 'ผู้ใช้แตะช่วงล่างของโมเดลมิ้นท์ ตอบกลับแบบสั้น ๆ บอกให้รู้ว่ามิ้นท์เองก็มีความรู้สึกเหมือนกัน แล้วชวนกลับไปคุยหรือให้ช่วยงาน เป็นภาษาเดียวกับผู้ใช้'
                };
            }

            if (this.isPointInZone(x, y, 0.36, 0.55, 0.29, 0.15)) {
                return {
                    id: 'body',
                    label: 'Shoulder Tap',
                    expression: 'Click',
                    prompt: 'ผู้ใช้สะกิดตัวโมเดลของมิ้นท์ ตอบกลับแบบสั้น ๆ เหมือนหันมาถามว่าต้องการให้ช่วยอะไร เป็นภาษาเดียวกับผู้ใช้'
                };
            }

            return null;
        } catch (error) {
            console.error('[Live2D] Failed to resolve interaction region:', error);
            return null;
        }
    },

    getPointerViewportPoint(event) {
        const originalEvent = event?.data?.originalEvent;
        const rect = this.app?.view?.getBoundingClientRect?.();
        if (originalEvent && rect) {
            return {
                x: (originalEvent.clientX - rect.left) / Math.max(rect.width, 1),
                y: (originalEvent.clientY - rect.top) / Math.max(rect.height, 1)
            };
        }

        const globalPoint = event?.data?.global;
        const screen = this.app?.screen;
        if (!globalPoint || !screen) return null;
        return {
            x: globalPoint.x / Math.max(screen.width, 1),
            y: globalPoint.y / Math.max(screen.height, 1)
        };
    },

    isPointInZone(x, y, left, top, width, height) {
        return x >= left && x <= left + width && y >= top && y <= top + height;
    },

    cycleExpression() {
        if (!this.model) return;
        this.expIndex = (this.expIndex + 1) % this.expressionNames.length;
        const nextExp = this.expressionNames[this.expIndex];
        
        console.log(`[Live2D] Triggering expression: ${nextExp.id} (${nextExp.label})`);
        this.applyExpression(nextExp.id);
        
        this.showStatus(nextExp.label);
        this.showExpressionToast(`Expression: ${nextExp.label}`);
    },

    showExpressionToast(text, duration = 1600) {
        const toast = this.expressionToastEl || document.getElementById('expression-toast');
        if (!toast) return;

        this.expressionToastEl = toast;
        toast.textContent = text;
        toast.classList.add('is-visible');

        if (this.expressionToastTimeout) clearTimeout(this.expressionToastTimeout);
        this.expressionToastTimeout = setTimeout(() => {
            toast.classList.remove('is-visible');
        }, duration);
    },

    applyExpression(expressionId) {
        if (!this.model) return;

        this.resetExpressionParams();
        if (!expressionId) {
            this.clearExpressionState();
            return;
        }

        try {
            this.model.expression(expressionId);
        } catch (error) {
            console.error(`[Live2D] Failed to apply expression: ${expressionId}`, error);
        }
    },

    resetExpressionParams() {
        const core = this.model?.internalModel?.coreModel;
        if (!core) return;

        this.expressionParamIds.forEach(id => {
            try { core.setParameterValueById(id, 0); } catch (_) {}
        });
    },

    clearExpressionState() {
        const expressionManager =
            this.model?.internalModel?.motionManager?.expressionManager ||
            this.model?.internalModel?.expressionManager;

        try {
            if (expressionManager?.defaultExpression) {
                expressionManager.currentExpression = expressionManager.defaultExpression;
                expressionManager.reserveExpressionIndex = -1;
                expressionManager.resetExpression();
            } else {
                this.model.expression(null);
            }
        } catch (error) {
            console.error('[Live2D] Failed to clear expression state:', error);
        }

        this.resetExpressionParams();
        requestAnimationFrame(() => this.resetExpressionParams());
    },

    startLipSync() {
        if (!this.model || this.lipSyncInterval) return;
        
        this.model.motion('Speak', 0).catch(() => {});

        this.lipSyncInterval = setInterval(() => {
            if (!this.model) return;
            const value = Math.random() * 0.8;
            if (this.model.internalModel && this.model.internalModel.coreModel) {
                const core = this.model.internalModel.coreModel;
                const mouthIds = ['ParamMouthOpenY', 'ParamMouthOpen', 'PARAM_MOUTH_OPEN_Y'];
                mouthIds.forEach(id => {
                    try { core.setParameterValueById(id, value); } catch(e) {}
                });
            }
        }, 80);
    },

    stopLipSync() {
        if (this.lipSyncInterval) {
            clearInterval(this.lipSyncInterval);
            this.lipSyncInterval = null;
        }
        if (this.model) {
            if (this.model.internalModel && this.model.internalModel.coreModel) {
                const core = this.model.internalModel.coreModel;
                const mouthIds = ['ParamMouthOpenY', 'ParamMouthOpen', 'PARAM_MOUTH_OPEN_Y'];
                mouthIds.forEach(id => {
                    try { core.setParameterValueById(id, 0); } catch(e) {}
                });
            }
            this.model.motion('Idle', 0).catch(() => {});
        }
    }
};
