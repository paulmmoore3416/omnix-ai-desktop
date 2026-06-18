<script lang="ts">
  import { onMount } from 'svelte';
  
  let { 
    emotion = 'idle' as 'idle' | 'thinking' | 'speaking' | 'working' | 'happy' | 'excited' | 'focused' | 'confused' | 'success' | 'error' | 'listening' | 'processing',
    isSpeaking = false,
    isWorking = false
  } = $props();
  
  // Smooth easing functions
  const easeInOutCubic = (t: number) => t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
  const easeOutElastic = (t: number) => {
    const c4 = (2 * Math.PI) / 3;
    return t === 0 ? 0 : t === 1 ? 1 : Math.pow(2, -10 * t) * Math.sin((t * 10 - 0.75) * c4) + 1;
  };
  
  // Character state with smooth interpolation
  let characterX = $state(50);
  let characterY = $state(50);
  let targetX = $state(50);
  let targetY = $state(50);
  let velocityX = $state(0);
  let velocityY = $state(0);
  let isWalking = $state(false);
  let walkCycle = $state(0);
  let facingDirection = $state<'left' | 'right'>('right');
  let currentActivity = $state<'idle' | 'walking' | 'jumping' | 'waving' | 'dancing' | 'working' | 'thinking' | 'flying'>('idle');
  let jumpOffset = $state(0);
  let isFlying = $state(false);
  let jetpackThrust = $state(1);
  let jetpackSputtering = $state(false);
  let flightPath = $state(0);
  let jumpVelocity = $state(0);
  let armRotationLeft = $state(0);
  let armRotationRight = $state(0);
  let headBob = $state(0);
  let bodySquash = $state(1);
  let eyeState = $state<'open' | 'blink' | 'happy' | 'focused' | 'confused'>('open');
  let breathingCycle = $state(0);
  let idleAnimation = $state(0);
  
  // Particle system
  let particleEffects = $state<Array<{
    x: number;
    y: number;
    vx: number;
    vy: number;
    life: number;
    maxLife: number;
    color: string;
    size: number;
  }>>([]);
  
  // Professional color palette
  const colorThemes = {
    idle: { 
      primary: '#667eea',
      secondary: '#764ba2',
      accent: '#f093fb',
      glow: 'rgba(102, 126, 234, 0.5)',
      shadow: 'rgba(102, 126, 234, 0.2)'
    },
    thinking: { 
      primary: '#4facfe',
      secondary: '#00f2fe',
      accent: '#43e97b',
      glow: 'rgba(79, 172, 254, 0.5)',
      shadow: 'rgba(79, 172, 254, 0.2)'
    },
    speaking: { 
      primary: '#fa709a',
      secondary: '#fee140',
      accent: '#30cfd0',
      glow: 'rgba(250, 112, 154, 0.5)',
      shadow: 'rgba(250, 112, 154, 0.2)'
    },
    working: { 
      primary: '#a8edea',
      secondary: '#fed6e3',
      accent: '#fbc2eb',
      glow: 'rgba(168, 237, 234, 0.5)',
      shadow: 'rgba(168, 237, 234, 0.2)'
    },
    happy: { 
      primary: '#ffecd2',
      secondary: '#fcb69f',
      accent: '#ff9a9e',
      glow: 'rgba(255, 236, 210, 0.5)',
      shadow: 'rgba(255, 236, 210, 0.2)'
    },
    excited: { 
      primary: '#ff6b6b',
      secondary: '#feca57',
      accent: '#48dbfb',
      glow: 'rgba(255, 107, 107, 0.6)',
      shadow: 'rgba(255, 107, 107, 0.2)'
    },
    focused: { 
      primary: '#a29bfe',
      secondary: '#6c5ce7',
      accent: '#fd79a8',
      glow: 'rgba(162, 155, 254, 0.5)',
      shadow: 'rgba(162, 155, 254, 0.2)'
    },
    confused: { 
      primary: '#95afc0',
      secondary: '#535c68',
      accent: '#dfe6e9',
      glow: 'rgba(149, 175, 192, 0.4)',
      shadow: 'rgba(149, 175, 192, 0.2)'
    },
    success: { 
      primary: '#55efc4',
      secondary: '#00b894',
      accent: '#81ecec',
      glow: 'rgba(85, 239, 196, 0.5)',
      shadow: 'rgba(85, 239, 196, 0.2)'
    },
    error: { 
      primary: '#ff7675',
      secondary: '#d63031',
      accent: '#fab1a0',
      glow: 'rgba(255, 118, 117, 0.5)',
      shadow: 'rgba(255, 118, 117, 0.2)'
    },
    listening: { 
      primary: '#74b9ff',
      secondary: '#0984e3',
      accent: '#a29bfe',
      glow: 'rgba(116, 185, 255, 0.5)',
      shadow: 'rgba(116, 185, 255, 0.2)'
    },
    processing: { 
      primary: '#e17055',
      secondary: '#fdcb6e',
      accent: '#6c5ce7',
      glow: 'rgba(225, 112, 85, 0.5)',
      shadow: 'rgba(225, 112, 85, 0.2)'
    }
  };
  
  let currentTheme = $derived(colorThemes[emotion]);
  
  // Spawn particles
  function spawnParticles(count: number, color: string) {
    const newParticles = Array.from({ length: count }, () => ({
      x: characterX,
      y: characterY - 15,
      vx: (Math.random() - 0.5) * 3,
      vy: -3 - Math.random() * 2,
      life: 60,
      maxLife: 60,
      color,
      size: 3 + Math.random() * 3
    }));
    particleEffects = [...particleEffects, ...newParticles];
  }
  
  onMount(() => {
    let animationFrame: number;
    let lastActivityChange = Date.now();
    let blinkTimer = Date.now();
    let lastTime = Date.now();
    
    const animate = () => {
      const now = Date.now();
      const deltaTime = Math.min((now - lastTime) / 16.67, 2); // Cap at 2x for stability
      lastTime = now;
      
      // Smooth breathing animation
      breathingCycle += 0.02 * deltaTime;
      const breathScale = 1 + Math.sin(breathingCycle) * 0.02;
      
      // Idle animation
      idleAnimation += 0.01 * deltaTime;
      
      // Natural blinking
      if (now - blinkTimer > 3000 + Math.random() * 2000) {
        eyeState = 'blink';
        setTimeout(() => {
          eyeState = emotion === 'happy' || emotion === 'excited' ? 'happy' : 
                     emotion === 'focused' ? 'focused' :
                     emotion === 'confused' ? 'confused' : 'open';
        }, 120);
        blinkTimer = now;
      }
      
      // Activity management
      if (now - lastActivityChange > 6000 && !isWorking && !isSpeaking && currentActivity === 'idle') {
        const activities: typeof currentActivity[] = ['walking', 'waving', 'dancing', 'flying'];
        const randomActivity = activities[Math.floor(Math.random() * activities.length)];
        
        if (randomActivity === 'walking') {
          targetX = 25 + Math.random() * 50;
          targetY = 45 + Math.random() * 15;
          isWalking = true;
          currentActivity = 'walking';
        } else if (randomActivity === 'flying') {
          isFlying = true;
          currentActivity = 'flying';
          targetX = 20 + Math.random() * 60;
          targetY = 15 + Math.random() * 25;
          jetpackThrust = 1;
          
          // Schedule sputtering events
          const sputterInterval = setInterval(() => {
            if (currentActivity === 'flying') {
              jetpackSputtering = true;
              jetpackThrust = 0.3;
              setTimeout(() => {
                jetpackSputtering = false;
                jetpackThrust = 1;
              }, 300 + Math.random() * 400);
            } else {
              clearInterval(sputterInterval);
            }
          }, 1500 + Math.random() * 2000);
          
          setTimeout(() => {
            if (currentActivity === 'flying') {
              isFlying = false;
              currentActivity = 'idle';
              targetY = 50;
            }
          }, 5000);
        } else {
          currentActivity = randomActivity;
          setTimeout(() => {
            if (currentActivity === randomActivity) currentActivity = 'idle';
          }, 2500);
        }
        
        lastActivityChange = now;
      }
      
      // Smooth movement with acceleration
      if (isWalking) {
        const dx = targetX - characterX;
        const dy = targetY - characterY;
        const distance = Math.sqrt(dx * dx + dy * dy);
        
        if (distance > 0.5) {
          const acceleration = 0.15;
          const maxSpeed = 0.8;
          const friction = 0.85;
          
          velocityX += (dx / distance) * acceleration * deltaTime;
          velocityY += (dy / distance) * acceleration * deltaTime;
          
          const speed = Math.sqrt(velocityX * velocityX + velocityY * velocityY);
          if (speed > maxSpeed) {
            velocityX = (velocityX / speed) * maxSpeed;
            velocityY = (velocityY / speed) * maxSpeed;
          }
          
          characterX += velocityX * deltaTime;
          characterY += velocityY * deltaTime;
          
          velocityX *= friction;
          velocityY *= friction;
          
          walkCycle += 0.15 * deltaTime;
          facingDirection = dx > 0 ? 'right' : 'left';
          headBob = Math.sin(walkCycle * 2) * 1.5;
        } else {
          isWalking = false;
          currentActivity = 'idle';
          walkCycle = 0;
          headBob = 0;
          velocityX = 0;
          velocityY = 0;
        }
      }
      
      // Activity-specific animations
      switch (currentActivity) {
        case 'flying':
          flightPath += 0.08 * deltaTime;
          const dx = targetX - characterX;
          const dy = targetY - characterY;
          const distance = Math.sqrt(dx * dx + dy * dy);
          
          if (distance > 1) {
            const speed = 0.6 * jetpackThrust;
            velocityX += (dx / distance) * speed * deltaTime;
            velocityY += (dy / distance) * speed * deltaTime;
            
            const maxSpeed = 1.2 * jetpackThrust;
            const currentSpeed = Math.sqrt(velocityX * velocityX + velocityY * velocityY);
            if (currentSpeed > maxSpeed) {
              velocityX = (velocityX / currentSpeed) * maxSpeed;
              velocityY = (velocityY / currentSpeed) * maxSpeed;
            }
            
            characterX += velocityX * deltaTime;
            characterY += velocityY * deltaTime;
            
            velocityX *= 0.95;
            velocityY *= 0.95;
            
            facingDirection = dx > 0 ? 'right' : 'left';
            headBob = Math.sin(flightPath * 3) * 2;
            bodySquash = 1 + Math.sin(flightPath * 2) * 0.03;
            armRotationLeft = -45 + Math.sin(flightPath * 2) * 10;
            armRotationRight = -45 - Math.sin(flightPath * 2) * 10;
            
            // Spawn jetpack particles
            if (Math.random() < 0.3 * jetpackThrust) {
              spawnParticles(jetpackSputtering ? 1 : 2, jetpackSputtering ? '#ff6b6b' : currentTheme.glow);
            }
          }
          break;
          
        case 'jumping':
          jumpVelocity += 0.8 * deltaTime;
          jumpOffset -= jumpVelocity * deltaTime;
          if (jumpOffset <= 0) {
            jumpOffset = 0;
            jumpVelocity = -15;
            bodySquash = 0.9;
            setTimeout(() => bodySquash = 1, 100);
          }
          bodySquash = 1 + (jumpOffset / 100) * 0.1;
          break;
          
        case 'waving':
          armRotationRight = Math.sin(now / 200) * 50;
          armRotationLeft = Math.sin(now / 400) * 10;
          break;
          
        case 'dancing':
          armRotationLeft = Math.sin(now / 180) * 35;
          armRotationRight = -Math.sin(now / 180) * 35;
          headBob = Math.sin(now / 250) * 2.5;
          bodySquash = 1 + Math.sin(now / 200) * 0.05;
          break;
          
        case 'working':
          headBob = Math.sin(now / 1500) * 1;
          armRotationRight = Math.sin(now / 300) * 15;
          break;
          
        case 'thinking':
          headBob = Math.sin(now / 2500) * 2;
          armRotationLeft = -20;
          break;
          
        case 'idle':
          if (!isWalking) {
            headBob = Math.sin(idleAnimation) * 0.5;
            armRotationLeft = Math.sin(idleAnimation * 0.5) * 3;
            armRotationRight = -Math.sin(idleAnimation * 0.5) * 3;
          }
          break;
      }
      
      // Update particles with physics
      particleEffects = particleEffects
        .map(p => ({
          ...p,
          x: p.x + p.vx * deltaTime,
          y: p.y + p.vy * deltaTime,
          vy: p.vy + 0.15 * deltaTime,
          vx: p.vx * 0.98,
          life: p.life - deltaTime
        }))
        .filter(p => p.life > 0);
      
      // Spawn particles for certain emotions
      if ((emotion === 'success' || emotion === 'excited') && Math.random() < 0.05) {
        spawnParticles(2, currentTheme.glow);
      }
      
      animationFrame = requestAnimationFrame(animate);
    };
    
    animate();
    
    // Emotion-based triggers
    $effect(() => {
      switch (emotion) {
        case 'excited':
          if (currentActivity === 'idle') {
            currentActivity = 'jumping';
            jumpVelocity = -15;
            spawnParticles(8, currentTheme.glow);
            setTimeout(() => {
              if (currentActivity === 'jumping') currentActivity = 'idle';
            }, 2000);
          }
          break;
        case 'happy':
        case 'success':
          if (currentActivity === 'idle') {
            currentActivity = 'dancing';
            spawnParticles(5, currentTheme.glow);
            setTimeout(() => {
              if (currentActivity === 'dancing') currentActivity = 'idle';
            }, 3000);
          }
          break;
        case 'thinking':
        case 'processing':
          if (!isWorking && !isSpeaking) {
            currentActivity = 'thinking';
          }
          break;
      }
    });
    
    $effect(() => {
      if (isWorking) {
        currentActivity = 'working';
      } else if (isSpeaking) {
        currentActivity = 'idle';
      }
    });
    
    return () => {
      if (animationFrame) cancelAnimationFrame(animationFrame);
    };
  });
</script>

<div class="avatar-stage">
  <!-- Ambient background -->
  <div class="ambient-bg" style="background: radial-gradient(circle at 50% 50%, {currentTheme.glow}, transparent 70%);"></div>
  
  <!-- Environment -->
  <div class="environment">
    <div class="ground"></div>
    <div class="decoration cloud" style="left: 15%; top: 12%;"></div>
    <div class="decoration cloud" style="left: 75%; top: 18%; animation-delay: 3s;"></div>
    <div class="decoration star" style="left: 25%; top: 8%;"></div>
    <div class="decoration star" style="left: 70%; top: 10%; animation-delay: 1s;"></div>
    <div class="decoration star" style="left: 50%; top: 15%; animation-delay: 2s;"></div>
  </div>
  
  <!-- Particles -->
  {#each particleEffects as particle}
    <div 
      class="particle"
      style="
        left: {particle.x}%;
        top: {particle.y}%;
        width: {particle.size}px;
        height: {particle.size}px;
        background: {particle.color};
        opacity: {particle.life / particle.maxLife};
        transform: scale({particle.life / particle.maxLife});
      "
    ></div>
  {/each}
  
  <!-- Character -->
  <div 
    class="character"
    style="
      left: {characterX}%;
      top: {characterY}%;
      transform: translateY({-jumpOffset}px) scaleX({facingDirection === 'left' ? -1 : 1});
    "
  >
    <!-- Shadow -->
    <div 
      class="shadow"
      style="
        opacity: {0.3 - jumpOffset / 100};
        transform: scale({1 + jumpOffset / 80});
      "
    ></div>
    
    <!-- Body -->
    <div 
      class="body" 
      style="
        background: linear-gradient(135deg, {currentTheme.primary}, {currentTheme.secondary});
        transform: scaleY({bodySquash});
      "
    >
      <!-- Glow effect -->
      <div 
        class="body-glow" 
        style="
          box-shadow: 0 0 40px {currentTheme.glow}, inset 0 0 20px {currentTheme.glow};
        "
      ></div>
      
      <!-- Shine overlay -->
      <div class="shine-overlay"></div>
      
      <!-- Face -->
      <div class="face" style="transform: translateY({headBob}px);">
        <!-- Eyes -->
        <div class="eyes">
          <div class="eye left" class:blink={eyeState === 'blink'} class:happy={eyeState === 'happy'}>
            <div class="eye-white"></div>
            <div class="pupil" style="background: {currentTheme.accent};"></div>
            <div class="eye-shine"></div>
            {#if eyeState === 'focused'}
              <div class="focus-line"></div>
            {/if}
          </div>
          <div class="eye right" class:blink={eyeState === 'blink'} class:happy={eyeState === 'happy'}>
            <div class="eye-white"></div>
            <div class="pupil" style="background: {currentTheme.accent};"></div>
            <div class="eye-shine"></div>
            {#if eyeState === 'focused'}
              <div class="focus-line"></div>
            {/if}
          </div>
        </div>
        
        <!-- Mouth -->
        <div 
          class="mouth" 
          class:speaking={isSpeaking} 
          class:happy={emotion === 'happy' || emotion === 'success'} 
          class:confused={emotion === 'confused'}
          style="border-color: {currentTheme.accent};"
        >
          {#if isSpeaking}
            <div class="speech-wave"></div>
          {/if}
        </div>
        
        <!-- Emotion indicators -->
        {#if emotion === 'thinking' || emotion === 'processing'}
          <div class="thought-bubble">
            <div class="thought-dot" style="background: {currentTheme.glow};"></div>
            <div class="thought-dot" style="background: {currentTheme.glow};"></div>
            <div class="thought-dot" style="background: {currentTheme.glow};"></div>
          </div>
        {/if}
        
        {#if emotion === 'confused'}
          <div class="question-mark" style="color: {currentTheme.accent};">?</div>
        {/if}
        
        {#if emotion === 'error'}
          <div class="error-symbol" style="color: {currentTheme.accent};">!</div>
        {/if}
      </div>
      
      <!-- Arms -->
      <div class="arms">
        <div 
          class="arm left"
          style="
            transform: rotate({armRotationLeft}deg);
            background: linear-gradient(180deg, {currentTheme.primary}, {currentTheme.secondary});
          "
        >
          <div class="hand" style="background: {currentTheme.accent};"></div>
        </div>
        <div 
          class="arm right"
          style="
            transform: rotate({armRotationRight}deg);
            background: linear-gradient(180deg, {currentTheme.primary}, {currentTheme.secondary});
          "
        >
          <div class="hand" style="background: {currentTheme.accent};"></div>
        </div>
      </div>
      
      <!-- Legs -->
      <div class="legs">
        <div 
          class="leg left"
          style="
            transform: rotate({isWalking ? Math.sin(walkCycle + Math.PI) * 20 : 0}deg);
            background: linear-gradient(180deg, {currentTheme.primary}, {currentTheme.secondary});
          "
        >
          <div class="foot" style="background: {currentTheme.accent};"></div>
        </div>
        <div 
          class="leg right"
          style="
            transform: rotate({isWalking ? Math.sin(walkCycle) * 20 : 0}deg);
            background: linear-gradient(180deg, {currentTheme.primary}, {currentTheme.secondary});
          "
        >
          <div class="foot" style="background: {currentTheme.accent};"></div>
        </div>
      </div>
    </div>
    
    <!-- Jetpack -->
    {#if isFlying}
      <div
        class="jetpack"
        style="
          background: linear-gradient(135deg, {currentTheme.primary}, {currentTheme.secondary});
          opacity: {jetpackThrust};
        "
      >
        <div class="jetpack-body" style="background: {currentTheme.accent};"></div>
        <div
          class="jetpack-flame left"
          class:sputtering={jetpackSputtering}
          style="background: linear-gradient(to bottom, {jetpackSputtering ? '#ff6b6b' : '#feca57'}, {jetpackSputtering ? '#d63031' : '#ff9a9e'});"
        ></div>
        <div
          class="jetpack-flame right"
          class:sputtering={jetpackSputtering}
          style="background: linear-gradient(to bottom, {jetpackSputtering ? '#ff6b6b' : '#feca57'}, {jetpackSputtering ? '#d63031' : '#ff9a9e'});"
        ></div>
        <div class="jetpack-glow" style="box-shadow: 0 0 20px {currentTheme.glow};"></div>
      </div>
    {/if}
    
    <!-- Activity indicators -->
    {#if isWorking}
      <div class="activity-indicator working">
        <div class="tool" style="color: {currentTheme.glow}; filter: drop-shadow(0 0 8px {currentTheme.glow});">⚙️</div>
      </div>
    {/if}
    
    {#if emotion === 'listening'}
      <div class="activity-indicator listening">
        <div class="sound-wave" style="background: {currentTheme.glow};"></div>
        <div class="sound-wave" style="background: {currentTheme.glow};"></div>
        <div class="sound-wave" style="background: {currentTheme.glow};"></div>
      </div>
    {/if}
  </div>
  
  <!-- Status text -->
  <div class="status-text" style="color: {currentTheme.accent}; text-shadow: 0 0 10px {currentTheme.glow};">
    {emotion === 'idle' ? 'Ready to assist' :
     emotion === 'thinking' ? 'Analyzing...' :
     emotion === 'speaking' ? 'Responding...' :
     emotion === 'working' ? 'Processing task...' :
     emotion === 'happy' ? 'Task completed!' :
     emotion === 'excited' ? 'Excellent!' :
     emotion === 'focused' ? 'Concentrating...' :
     emotion === 'confused' ? 'Clarifying...' :
     emotion === 'success' ? 'Success!' :
     emotion === 'error' ? 'Error occurred' :
     emotion === 'listening' ? 'Listening...' :
     emotion === 'processing' ? 'Computing...' : ''}
  </div>
</div>

<style>
  .avatar-stage {
    position: relative;
    width: 400px;
    height: 300px;
    overflow: hidden;
    background: linear-gradient(to bottom, #0f0f1e 0%, #1a1a2e 50%, #16213e 100%);
    border-radius: 24px;
    box-shadow: 
      0 20px 60px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.1);
  }
  
  .ambient-bg {
    position: absolute;
    inset: 0;
    opacity: 0.3;
    filter: blur(60px);
    animation: ambient-pulse 4s ease-in-out infinite;
  }
  
  .environment {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  
  .ground {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 35%;
    background: linear-gradient(to bottom, transparent, rgba(255, 255, 255, 0.03));
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }
  
  .decoration {
    position: absolute;
    pointer-events: none;
  }
  
  .cloud {
    width: 50px;
    height: 16px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    animation: float-cloud 8s ease-in-out infinite;
    backdrop-filter: blur(2px);
  }
  
  .cloud::before,
  .cloud::after {
    content: '';
    position: absolute;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 50%;
  }
  
  .cloud::before {
    width: 20px;
    height: 20px;
    top: -8px;
    left: 8px;
  }
  
  .cloud::after {
    width: 24px;
    height: 24px;
    top: -10px;
    right: 8px;
  }
  
  .star {
    width: 3px;
    height: 3px;
    background: white;
    border-radius: 50%;
    box-shadow: 0 0 4px rgba(255, 255, 255, 0.8);
    animation: twinkle 3s ease-in-out infinite;
  }
  
  .particle {
    position: absolute;
    border-radius: 50%;
    pointer-events: none;
    filter: blur(1px);
  }
  
  .character {
    position: absolute;
    width: 80px;
    height: 100px;
    transition: left 0.05s linear, top 0.05s linear;
    transform-origin: bottom center;
    will-change: transform, left, top;
  }
  
  .shadow {
    position: absolute;
    bottom: -8px;
    left: 50%;
    transform: translateX(-50%);
    width: 45px;
    height: 8px;
    background: radial-gradient(ellipse, rgba(0, 0, 0, 0.4), transparent);
    border-radius: 50%;
    filter: blur(3px);
    transition: opacity 0.2s ease, transform 0.2s ease;
  }
  
  .body {
    position: relative;
    width: 48px;
    height: 58px;
    margin: 0 auto;
    border-radius: 24px 24px 16px 16px;
    transition: background 0.5s ease, transform 0.1s ease;
    overflow: hidden;
  }
  
  .body-glow {
    position: absolute;
    inset: -3px;
    border-radius: inherit;
    opacity: 0.7;
    animation: pulse-glow 3s ease-in-out infinite;
  }
  
  .shine-overlay {
    position: absolute;
    inset: 0;
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.3) 0%, transparent 50%);
    border-radius: inherit;
    opacity: 0.6;
  }
  
  .face {
    position: relative;
    width: 100%;
    height: 38px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 7px;
    padding-top: 12px;
    transition: transform 0.1s ease;
  }
  
  .eyes {
    display: flex;
    gap: 14px;
  }
  
  .eye {
    position: relative;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    transition: all 0.15s ease;
    overflow: hidden;
  }
  
  .eye-white {
    position: absolute;
    inset: 0;
    background: white;
    border-radius: 50%;
  }
  
  .eye.blink {
    height: 2px;
  }
  
  .eye.blink .eye-white,
  .eye.blink .pupil,
  .eye.blink .eye-shine {
    opacity: 0;
  }
  
  .eye.happy {
    height: 7px;
    border-radius: 0 0 11px 11px;
  }
  
  .pupil {
    position: absolute;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    transition: all 0.2s ease;
  }
  
  .eye-shine {
    position: absolute;
    width: 3px;
    height: 3px;
    background: rgba(255, 255, 255, 0.9);
    border-radius: 50%;
    top: 25%;
    left: 30%;
  }
  
  .focus-line {
    position: absolute;
    width: 2px;
    height: 14px;
    background: rgba(0, 0, 0, 0.2);
    top: -3px;
    left: 50%;
    transform: translateX(-50%);
  }
  
  .mouth {
    width: 18px;
    height: 9px;
    border: 2.5px solid;
    border-top: none;
    border-radius: 0 0 9px 9px;
    transition: all 0.2s ease;
    position: relative;
  }
  
  .mouth.speaking {
    animation: speak 0.25s ease-in-out infinite;
  }
  
  .mouth.happy {
    width: 22px;
    height: 11px;
    border-radius: 0 0 11px 11px;
  }
  
  .mouth.confused {
    border-radius: 9px 9px 0 0;
    border-top: 2.5px solid;
    border-bottom: none;
  }
  
  .speech-wave {
    position: absolute;
    width: 100%;
    height: 100%;
    background: rgba(0, 0, 0, 0.1);
    border-radius: inherit;
    animation: speech-pulse 0.25s ease-in-out infinite;
  }
  
  .thought-bubble {
    position: absolute;
    top: -28px;
    right: -22px;
    display: flex;
    gap: 4px;
  }
  
  .thought-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    animation: think 1.2s ease-in-out infinite;
    box-shadow: 0 0 6px currentColor;
  }
  
  .thought-dot:nth-child(2) { animation-delay: 0.2s; }
  .thought-dot:nth-child(3) { animation-delay: 0.4s; }
  
  .question-mark,
  .error-symbol {
    position: absolute;
    top: -32px;
    right: -18px;
    font-size: 22px;
    font-weight: bold;
    animation: float-indicator 1.5s ease-in-out infinite;
    filter: drop-shadow(0 0 8px currentColor);
  }
  
  .arms {
    position: absolute;
    top: 16px;
    width: 100%;
  }
  
  .arm {
    position: absolute;
    width: 9px;
    height: 26px;
    border-radius: 5px;
    transform-origin: top center;
    transition: transform 0.15s ease;
    box-shadow: inset 0 -2px 4px rgba(0, 0, 0, 0.2);
  }
  
  .arm.left {
    left: -6px;
  }
  
  .arm.right {
    right: -6px;
  }
  
  .hand {
    position: absolute;
    bottom: -6px;
    left: 50%;
    transform: translateX(-50%);
    width: 11px;
    height: 11px;
    border-radius: 50%;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  }
  
  .legs {
    position: absolute;
    bottom: -26px;
    width: 100%;
    display: flex;
    justify-content: center;
    gap: 12px;
  }
  
  .leg {
    width: 11px;
    height: 26px;
    border-radius: 6px;
    transform-origin: top center;
    transition: transform 0.1s ease;
    box-shadow: inset 0 -2px 4px rgba(0, 0, 0, 0.2);
  }
  
  .foot {
    position: absolute;
    bottom: -6px;
    left: 50%;
    transform: translateX(-50%);
    width: 16px;
    height: 9px;
    border-radius: 5px;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  }
  
  .jetpack {
    position: absolute;
    top: 20px;
    left: 50%;
    transform: translateX(-50%);
    width: 32px;
    height: 38px;
    border-radius: 8px;
    z-index: -1;
    transition: opacity 0.3s ease;
    box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3);
  }
  
  .jetpack-body {
    position: absolute;
    top: 4px;
    left: 50%;
    transform: translateX(-50%);
    width: 20px;
    height: 28px;
    border-radius: 6px;
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.2);
  }
  
  .jetpack-flame {
    position: absolute;
    bottom: -18px;
    width: 8px;
    height: 20px;
    border-radius: 0 0 4px 4px;
    animation: jetpack-flame 0.15s ease-in-out infinite;
    filter: blur(1px);
    box-shadow: 0 0 10px currentColor;
  }
  
  .jetpack-flame.left {
    left: 6px;
  }
  
  .jetpack-flame.right {
    right: 6px;
  }
  
  .jetpack-flame.sputtering {
    animation: sputter-flame 0.1s ease-in-out infinite;
    opacity: 0.6;
  }
  
  .jetpack-glow {
    position: absolute;
    inset: -4px;
    border-radius: inherit;
    opacity: 0.5;
    animation: jetpack-glow 0.3s ease-in-out infinite;
  }
  
  .activity-indicator {
    position: absolute;
    top: -35px;
    left: 50%;
    transform: translateX(-50%);
  }
  
  .tool {
    font-size: 22px;
    animation: rotate-tool 3s linear infinite;
  }
  
  .listening {
    display: flex;
    gap: 4px;
  }
  
  .sound-wave {
    width: 4px;
    height: 18px;
    border-radius: 2px;
    animation: wave 0.7s ease-in-out infinite;
    box-shadow: 0 0 8px currentColor;
  }
  
  .sound-wave:nth-child(2) { animation-delay: 0.2s; }
  .sound-wave:nth-child(3) { animation-delay: 0.4s; }
  
  .status-text {
    position: absolute;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.5px;
    white-space: nowrap;
    animation: fade-in 0.5s ease;
  }
  
  @keyframes ambient-pulse {
    0%, 100% { opacity: 0.2; }
    50% { opacity: 0.4; }
  }
  
  @keyframes float-cloud {
    0%, 100% { transform: translateY(0) translateX(0); }
    50% { transform: translateY(-8px) translateX(5px); }
  }
  
  @keyframes twinkle {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.4; transform: scale(0.8); }
  }
  
  @keyframes pulse-glow {
    0%, 100% { opacity: 0.6; }
    50% { opacity: 0.9; }
  }
  
  @keyframes speak {
    0%, 100% { height: 9px; }
    50% { height: 13px; }
  }
  
  @keyframes speech-pulse {
    0%, 100% { opacity: 0.1; }
    50% { opacity: 0.3; }
  }
  
  @keyframes think {
    0%, 100% { opacity: 0.4; transform: translateY(0) scale(1); }
    50% { opacity: 1; transform: translateY(-6px) scale(1.2); }
  }
  
  @keyframes float-indicator {
    0%, 100% { transform: translateY(0); }
    50% { transform: translateY(-5px); }
  }
  
  @keyframes rotate-tool {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
  
  @keyframes wave {
    0%, 100% { height: 12px; opacity: 0.6; }
    50% { height: 22px; opacity: 1; }
  }
  
  @keyframes fade-in {
    from { opacity: 0; transform: translateX(-50%) translateY(10px); }
    to { opacity: 1; transform: translateX(-50%) translateY(0); }
  }
  
  @keyframes jetpack-flame {
    0%, 100% { height: 20px; opacity: 1; }
    50% { height: 24px; opacity: 0.9; }
  }
  
  @keyframes sputter-flame {
    0%, 100% { height: 12px; opacity: 0.4; transform: scaleX(0.8); }
    50% { height: 8px; opacity: 0.2; transform: scaleX(0.6); }
  }
  
  @keyframes jetpack-glow {
    0%, 100% { opacity: 0.4; }
    50% { opacity: 0.7; }
  }
</style>