<script lang="ts">
  import { onMount } from 'svelte';
  
  export let emotion: 'idle' | 'thinking' | 'speaking' | 'working' | 'happy' | 'excited' | 'focused' | 'confused' | 'success' | 'error' | 'listening' | 'processing' = 'idle';
  export let isSpeaking = false;
  export let isWorking = false;
  
  let mouthOpen = $state(0);
  let eyeGlow = $state(1);
  let eyePosition = $state({ left: 0, right: 0 });
  let blinkState = $state(false);
  let leftArmAngle = $state(0);
  let rightArmAngle = $state(0);
  let showArms = $state(false);
  let particleCount = $state(0);
  let headTilt = $state(0);
  let bodyScale = $state(1);
  
  // Color schemes based on state
  const colorSchemes = {
    idle: { primary: '#00d4ff', secondary: '#8b5cf6', glow: '#00d4ff80' },
    thinking: { primary: '#fbbf24', secondary: '#f59e0b', glow: '#fbbf2480' },
    speaking: { primary: '#00d4ff', secondary: '#06b6d4', glow: '#00d4ff80' },
    working: { primary: '#10b981', secondary: '#059669', glow: '#10b98180' },
    happy: { primary: '#fbbf24', secondary: '#f59e0b', glow: '#fbbf2480' },
    excited: { primary: '#f43f5e', secondary: '#e11d48', glow: '#f43f5e80' },
    focused: { primary: '#8b5cf6', secondary: '#7c3aed', glow: '#8b5cf680' },
    confused: { primary: '#6b7280', secondary: '#4b5563', glow: '#6b728080' },
    success: { primary: '#10b981', secondary: '#059669', glow: '#10b98180' },
    error: { primary: '#ef4444', secondary: '#dc2626', glow: '#ef444480' },
    listening: { primary: '#06b6d4', secondary: '#0891b2', glow: '#06b6d480' },
    processing: { primary: '#a855f7', secondary: '#9333ea', glow: '#a855f780' }
  };
  
  let currentColors = $derived(colorSchemes[emotion]);
  
  onMount(() => {
    // Idle animations
    const idleInterval = setInterval(() => {
      if (emotion === 'idle') {
        eyeGlow = 0.8 + Math.random() * 0.2;
        headTilt = (Math.random() - 0.5) * 5;
      }
    }, 2000);
    
    // Blinking
    const blinkInterval = setInterval(() => {
      blinkState = true;
      setTimeout(() => blinkState = false, 150);
    }, 3000 + Math.random() * 2000);
    
    // Eye movement (looking around)
    const eyeMovementInterval = setInterval(() => {
      if (!isWorking && emotion !== 'focused') {
        eyePosition = {
          left: (Math.random() - 0.5) * 10,
          right: (Math.random() - 0.5) * 10
        };
      }
    }, 2000);
    
    // Arm animations when working
    const armInterval = setInterval(() => {
      if (isWorking) {
        showArms = true;
        leftArmAngle = Math.sin(Date.now() / 500) * 20;
        rightArmAngle = Math.cos(Date.now() / 500) * 20;
      } else {
        showArms = false;
        leftArmAngle = 0;
        rightArmAngle = 0;
      }
    }, 50);
    
    // Particle effects
    const particleInterval = setInterval(() => {
      if (emotion === 'processing' || emotion === 'thinking') {
        particleCount = (particleCount + 1) % 20;
      }
    }, 100);
    
    return () => {
      clearInterval(idleInterval);
      clearInterval(blinkInterval);
      clearInterval(eyeMovementInterval);
      clearInterval(armInterval);
      clearInterval(particleInterval);
    };
  });
  
  // Reactive animations based on emotion
  $effect(() => {
    switch (emotion) {
      case 'happy':
        bodyScale = 1.05;
        mouthOpen = 0.8;
        break;
      case 'excited':
        bodyScale = 1.1;
        mouthOpen = 1;
        break;
      case 'thinking':
        headTilt = 15;
        eyeGlow = 0.6;
        break;
      case 'confused':
        headTilt = -15;
        eyeGlow = 0.5;
        break;
      case 'focused':
        eyeGlow = 1;
        eyePosition = { left: 0, right: 0 };
        break;
      case 'success':
        bodyScale = 1.08;
        mouthOpen = 0.9;
        break;
      case 'error':
        bodyScale = 0.95;
        mouthOpen = 0.3;
        break;
      default:
        bodyScale = 1;
        headTilt = 0;
    }
  });
  
  // Speaking animation
  $effect(() => {
    if (isSpeaking) {
      const interval = setInterval(() => {
        mouthOpen = Math.random() * 0.8 + 0.2;
      }, 100);
      return () => clearInterval(interval);
    } else {
      mouthOpen = 0;
    }
  });
</script>

<div class="relative animate-float" style="transform: scale({bodyScale}); transition: transform 0.3s ease;">
  <div class="w-64 h-64 relative" style="transform: rotate({headTilt}deg); transition: transform 0.5s ease;">
    <!-- Outer glow ring with dynamic color -->
    <div 
      class="absolute inset-0 rounded-full blur-3xl animate-pulse-glow"
      style="background-color: {currentColors.glow};"
    ></div>
    
    <!-- Particle effects -->
    {#if emotion === 'processing' || emotion === 'thinking'}
      {#each Array(8) as _, i}
        <div 
          class="absolute w-2 h-2 rounded-full"
          style="
            background-color: {currentColors.primary};
            left: {50 + Math.cos(i * Math.PI / 4 + particleCount * 0.1) * 40}%;
            top: {50 + Math.sin(i * Math.PI / 4 + particleCount * 0.1) * 40}%;
            opacity: {0.3 + Math.sin(particleCount * 0.2) * 0.3};
            transition: all 0.1s ease;
          "
        ></div>
      {/each}
    {/if}
    
    <!-- Main avatar circle -->
    <div 
      class="absolute inset-4 rounded-full border-4 shadow-2xl flex items-center justify-center transition-all duration-300"
      style="
        background: linear-gradient(135deg, {currentColors.primary}, {currentColors.secondary});
        border-color: {currentColors.primary}80;
        box-shadow: 0 0 40px {currentColors.glow};
      "
    >
      <!-- Arms (show when working) -->
      {#if showArms}
        <!-- Left arm -->
        <div 
          class="absolute left-0 top-1/2 w-16 h-4 rounded-full origin-right transition-all duration-100"
          style="
            background: linear-gradient(90deg, {currentColors.primary}, {currentColors.secondary});
            transform: translateX(-100%) translateY(-50%) rotate({leftArmAngle}deg);
            opacity: 0.8;
          "
        >
          <!-- Hand -->
          <div 
            class="absolute left-0 top-1/2 w-6 h-6 rounded-full -translate-y-1/2 -translate-x-1/2"
            style="background-color: {currentColors.primary};"
          ></div>
        </div>
        
        <!-- Right arm -->
        <div 
          class="absolute right-0 top-1/2 w-16 h-4 rounded-full origin-left transition-all duration-100"
          style="
            background: linear-gradient(90deg, {currentColors.secondary}, {currentColors.primary});
            transform: translateX(100%) translateY(-50%) rotate({-rightArmAngle}deg);
            opacity: 0.8;
          "
        >
          <!-- Hand -->
          <div 
            class="absolute right-0 top-1/2 w-6 h-6 rounded-full -translate-y-1/2 translate-x-1/2"
            style="background-color: {currentColors.primary};"
          ></div>
        </div>
      {/if}
      
      <!-- Face -->
      <div class="relative w-full h-full flex items-center justify-center">
        <!-- Eyes -->
        <div 
          class="absolute top-1/3 left-1/3 w-8 h-8 rounded-full shadow-lg transition-all duration-200"
          style="
            background-color: {currentColors.primary};
            box-shadow: 0 0 20px {currentColors.glow};
            opacity: {blinkState ? 0.1 : eyeGlow};
            transform: translateX(-50%) translateX({eyePosition.left}px) scaleY({blinkState ? 0.1 : 1});
          "
        ></div>
        <div 
          class="absolute top-1/3 right-1/3 w-8 h-8 rounded-full shadow-lg transition-all duration-200"
          style="
            background-color: {currentColors.primary};
            box-shadow: 0 0 20px {currentColors.glow};
            opacity: {blinkState ? 0.1 : eyeGlow};
            transform: translateX(50%) translateX({eyePosition.right}px) scaleY({blinkState ? 0.1 : 1});
          "
        ></div>
        
        <!-- Eyebrows (for expressions) -->
        {#if emotion === 'confused' || emotion === 'thinking'}
          <div 
            class="absolute top-1/4 left-1/3 w-10 h-1 rounded-full transition-all duration-300"
            style="
              background-color: {currentColors.primary};
              transform: translateX(-50%) rotate({emotion === 'confused' ? -15 : 5}deg);
            "
          ></div>
          <div 
            class="absolute top-1/4 right-1/3 w-10 h-1 rounded-full transition-all duration-300"
            style="
              background-color: {currentColors.primary};
              transform: translateX(50%) rotate({emotion === 'confused' ? 15 : -5}deg);
            "
          ></div>
        {/if}
        
        <!-- Mouth -->
        <div class="absolute bottom-1/3 left-1/2 -translate-x-1/2">
          {#if emotion === 'happy' || emotion === 'excited' || emotion === 'success'}
            <!-- Smile -->
            <svg width="64" height="32" viewBox="0 0 64 32" class="transition-all duration-300">
              <path 
                d="M 8 8 Q 32 {24 + mouthOpen * 8} 56 8" 
                stroke="{currentColors.primary}" 
                stroke-width="3" 
                fill="none" 
                stroke-linecap="round"
              />
            </svg>
          {:else if emotion === 'error' || emotion === 'confused'}
            <!-- Frown or neutral -->
            <svg width="64" height="32" viewBox="0 0 64 32" class="transition-all duration-300">
              <path 
                d="M 8 24 Q 32 {16 - mouthOpen * 4} 56 24" 
                stroke="{currentColors.primary}" 
                stroke-width="3" 
                fill="none" 
                stroke-linecap="round"
              />
            </svg>
          {:else}
            <!-- Default mouth -->
            <div 
              class="w-16 h-2 rounded-full transition-all duration-100"
              style="
                background-color: {currentColors.primary};
                transform: scaleY({1 + mouthOpen * 3});
                box-shadow: 0 0 10px {currentColors.glow};
              "
            ></div>
          {/if}
        </div>

        <!-- Neural network pattern -->
        <svg class="absolute inset-0 w-full h-full opacity-20 transition-opacity duration-300" viewBox="0 0 100 100">
          <circle cx="50" cy="50" r="40" fill="none" stroke="{currentColors.primary}" stroke-width="0.5" class="animate-spin-slow"/>
          <circle cx="50" cy="50" r="30" fill="none" stroke="{currentColors.primary}" stroke-width="0.5"/>
          <circle cx="50" cy="50" r="20" fill="none" stroke="{currentColors.primary}" stroke-width="0.5" class="animate-spin-reverse"/>
          {#if emotion === 'processing' || emotion === 'working'}
            <circle cx="50" cy="50" r="35" fill="none" stroke="{currentColors.secondary}" stroke-width="0.5" class="animate-pulse"/>
          {/if}
        </svg>
      </div>
    </div>
  </div>
</div>

<style>
  @keyframes spin-slow {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
  
  @keyframes spin-reverse {
    from { transform: rotate(360deg); }
    to { transform: rotate(0deg); }
  }
  
  .animate-spin-slow {
    animation: spin-slow 20s linear infinite;
  }
  
  .animate-spin-reverse {
    animation: spin-reverse 15s linear infinite;
  }
</style>