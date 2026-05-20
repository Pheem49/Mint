/**
 * Live2DManager - Encapsulates Live2D model loading, fitting, and lip-sync logic.
 */
window.Live2DManager = {
    app: null,
    model: null,
    resizeObserver: null,
    lipSyncInterval: null,
    expIndex: 0,
    interactionEnabled: true,
    interactionStorageKey: 'mint-model-interaction-enabled',
    accessoryStorageKey: 'mint-live2d-accessories',
    activeAccessories: {},
    accessoryOrder: ['glasses', 'pen', 'cat'],
    accessoryParams: {
        glasses: { paramId: 'Param96', label: 'Glasses' },
        pen: { paramId: 'Param68', label: 'Pen' },
        cat: { paramId: 'Param54', label: 'Cat Filter' }
    },
    pointerTrackingEnabled: true,
    pointerTrackingFrame: null,
    pointerTracking: {
        targetX: 0,
        targetY: 0,
        currentX: 0,
        currentY: 0,
        lastMoveAt: 0
    },
    pointerTrackingConfig: {
        focusX: 0.35,
        focusY: 0.35,
        rangeX: 0.35,
        rangeY: 0.35,
        smoothing: 0.18
    },
    baseModelPosition: null,
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
        this.interactionEnabled = this.getSavedInteractionEnabled();
        this.activeAccessories = this.getSavedAccessories();
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
                autoInteract: false
            });
            this.expressionToastEl = document.getElementById('expression-toast');

            this.model.anchor.set(0.5, 0.5);
            this.app.stage.addChild(this.model);

            // -- Interaction Setup --
            this.setInteractionEnabled(this.interactionEnabled);
            this.setupPointerTracking(mountEl);
            this.applyAccessories();

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
                const scale = Math.min(widthScale, heightScale) * 1.85;

                this.model.scale.set(scale);
                // Adjusted Y offset to 1.0 as requested
                this.baseModelPosition = {
                    x: mountWidth / 2,
                    y: mountHeight / 2 + mountHeight * 0.55
                };
                this.applyModelFollowOffset();
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
        if (!this.model || !this.interactionEnabled) return;

        const now = Date.now();
        if (now - this.lastInteractionAt < 3000) return;

        const region = this.getInteractionRegion(event);
        if (!region) return;
        this.lastInteractionAt = now;
        const expressionId = region.expression || 'Click';

        console.log(`[Live2D] Interaction: ${region.id}`, region);
        this.applyExpression(expressionId);
        this.showStatus(region.label, 2500);

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

            if (this.isPointInZone(x, y, 0.38, 0.40, 0.24, 0.115)) {
                return {
                    id: 'face',
                    label: 'Cat Ears',
                    expression: 'CatFilter',
                    prompt: 'The user poked Mint model on the cheek. Reply briefly, shyly or with a light tease. Use the same language as the user’s recent conversation; do not switch to Thai unless the user has been speaking Thai.'
                };
            }

            if (this.isPointInZone(x, y, 0.34, 0.255, 0.32, 0.15)) {
                return {
                    id: 'head',
                    label: 'Head Pat',
                    expression: 'Dazed',
                    prompt: 'The user patted Mint model on the head. Reply briefly in a cute, slightly shy way. Use the same language as the user’s recent conversation; do not switch to Thai unless the user has been speaking Thai.'
                };
            }

            const isLeftHand = this.isPointInZone(x, y, 0.22, 0.68, 0.20, 0.16);
            const isRightHand = this.isPointInZone(x, y, 0.61, 0.68, 0.19, 0.16);
            if (isLeftHand || isRightHand) {
                return {
                    id: isLeftHand ? 'left-hand' : 'right-hand',
                    label: 'Hand Tap',
                    expression: 'Pen',
                    prompt: 'The user tapped Mint model’s hand. Reply briefly as if ready to help or take a request. Use the same language as the user’s recent conversation; do not switch to Thai unless the user has been speaking Thai.'
                };
            }

            if (this.isPointInZone(x, y, 0.38, 0.77, 0.30, 0.23)) {
                return {
                    id: 'lower-body',
                    label: 'Careful',
                    expression: 'Photo',
                    prompt: 'The user touched the lower body area of Mint model. Reply briefly in a shy, playful way, similar to “hehe~ what are you playing at, that makes me blush,” then gently invite the user back to chatting or work. Use the same language as the user’s recent conversation; do not switch to Thai unless the user has been speaking Thai.'
                };
            }

            if (this.isPointInZone(x, y, 0.37, 0.555, 0.29, 0.14)) {
                return {
                    id: 'body',
                    label: 'Shoulder Tap',
                    expression: 'Click',
                    prompt: 'The user tapped Mint model’s body or shoulder. Reply briefly as if turning toward the user and asking what they need help with. Use the same language as the user’s recent conversation; do not switch to Thai unless the user has been speaking Thai.'
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

    setInteractionEnabled(isEnabled, persist = false) {
        this.interactionEnabled = Boolean(isEnabled);
        if (persist) {
            this.saveInteractionEnabled(this.interactionEnabled);
        }
        if (!this.model) return;

        this.model.interactive = this.interactionEnabled;
        this.model.buttonMode = this.interactionEnabled;
    },

    getSavedInteractionEnabled() {
        try {
            return localStorage.getItem(this.interactionStorageKey) !== 'false';
        } catch (_) {
            return true;
        }
    },

    saveInteractionEnabled(isEnabled) {
        try {
            localStorage.setItem(this.interactionStorageKey, String(Boolean(isEnabled)));
        } catch (_) {}
    },

    setupPointerTracking(mountEl) {
        if (!mountEl) return;

        window.addEventListener('mousemove', (event) => this.updatePointerTrackingTarget(event, mountEl));

        if (!this.pointerTrackingFrame) {
            this.pointerTrackingFrame = () => this.updatePointerTracking();
            this.app?.ticker?.add(this.pointerTrackingFrame);
        }
    },

    updatePointerTrackingTarget(event, mountEl) {
        if (!this.pointerTrackingEnabled || !mountEl) return;

        const rect = {
            left: 0,
            top: 0,
            width: window.innerWidth || mountEl.getBoundingClientRect().width,
            height: window.innerHeight || mountEl.getBoundingClientRect().height
        };
        const config = this.pointerTrackingConfig;
        const centerX = rect.left + rect.width * config.focusX;
        const centerY = rect.top + rect.height * config.focusY;
        const rangeX = Math.max(rect.width * config.rangeX, 1);
        const rangeY = Math.max(rect.height * config.rangeY, 1);

        this.pointerTracking.targetX = this.clamp((event.clientX - centerX) / rangeX, -1, 1);
        this.pointerTracking.targetY = this.clamp((event.clientY - centerY) / rangeY, -1, 1);
        this.pointerTracking.lastMoveAt = performance.now();
    },

    resetPointerTrackingTarget() {
        this.pointerTracking.targetX = 0;
        this.pointerTracking.targetY = 0;
    },

    updatePointerTracking() {
        if (!this.model || !this.pointerTrackingEnabled) return;

        const tracking = this.pointerTracking;
        const smoothing = this.pointerTrackingConfig.smoothing;
        tracking.currentX += (tracking.targetX - tracking.currentX) * smoothing;
        tracking.currentY += (tracking.targetY - tracking.currentY) * smoothing;

        const x = tracking.currentX;
        const y = tracking.currentY;
        const core = this.model?.internalModel?.coreModel;
        if (!core) return;

        this.setLive2DParam(core, 'ParamAngleX', x * 18);
        this.setLive2DParam(core, 'ParamAngleY', -y * 14);
        this.setLive2DParam(core, 'ParamAngleZ', -x * 5);
        this.setLive2DParam(core, 'ParamEyeBallX', x * 1.45);
        this.setLive2DParam(core, 'ParamEyeBallY', -y * 1.35);
        this.setLive2DParam(core, 'Param49', x * 7);
        this.setLive2DParam(core, 'Param51', -y * 5);
        this.setLive2DParam(core, 'Param50', -x * 3);
        this.applyModelFollowOffset();
    },

    applyModelFollowOffset() {
        if (!this.model || !this.baseModelPosition) return;

        const x = this.pointerTracking.currentX || 0;
        const y = this.pointerTracking.currentY || 0;
        this.model.position.set(
            this.baseModelPosition.x + x * 22,
            this.baseModelPosition.y + y * 16
        );
    },

    setLive2DParam(core, id, value) {
        try {
            core.setParameterValueById(id, value);
        } catch (_) {}
    },

    clamp(value, min, max) {
        return Math.max(min, Math.min(max, value));
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
            requestAnimationFrame(() => this.applyAccessories());
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
        requestAnimationFrame(() => {
            this.resetExpressionParams();
            this.applyAccessories();
        });
    },

    setAccessory(accessoryId, isEnabled, persist = false) {
        if (!this.accessoryParams[accessoryId]) return;

        this.activeAccessories[accessoryId] = Boolean(isEnabled);
        if (persist) {
            this.saveAccessories();
        }
        this.applyAccessory(accessoryId);
    },

    setExclusiveAccessory(accessoryId, persist = false) {
        const nextAccessoryId = this.accessoryParams[accessoryId] ? accessoryId : null;
        Object.keys(this.accessoryParams).forEach(id => {
            this.activeAccessories[id] = id === nextAccessoryId;
        });
        if (persist) {
            this.saveAccessories();
        }
        this.applyAccessories();
        return nextAccessoryId;
    },

    getActiveAccessoryId() {
        return this.accessoryOrder.find(id => this.activeAccessories[id]) || null;
    },

    applyAccessories() {
        Object.keys(this.accessoryParams).forEach(accessoryId => {
            this.applyAccessory(accessoryId);
        });
    },

    applyAccessory(accessoryId) {
        const accessory = this.accessoryParams[accessoryId];
        const core = this.model?.internalModel?.coreModel;
        if (!accessory || !core) return;

        this.setLive2DParam(core, accessory.paramId, this.activeAccessories[accessoryId] ? 1 : 0);
    },

    getSavedAccessories() {
        try {
            return JSON.parse(localStorage.getItem(this.accessoryStorageKey) || '{}') || {};
        } catch (_) {
            return {};
        }
    },

    saveAccessories() {
        try {
            localStorage.setItem(this.accessoryStorageKey, JSON.stringify(this.activeAccessories));
        } catch (_) {}
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
