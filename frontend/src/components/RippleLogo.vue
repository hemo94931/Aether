<template>
  <Transition name="logo-fade">
    <!-- Adaptive Aether logo using external SVG with CSS-based dark mode -->
    <!-- Animation sequence: stroke outline -> fill color -> ripple breathing -->
    <div
      v-if="type === 'aether' && useAdaptive"
      :key="`aether-adaptive-${animationKey}`"
      class="aether-adaptive-container"
      :style="{ '--anim-delay': `${animDelay}ms` }"
    >
      <!-- Definitions for gradient and glow -->
      <svg
        style="position: absolute; width: 0; height: 0; overflow: hidden;"
        aria-hidden="true"
      >
        <defs>
          <linearGradient
            id="adaptive-aether-gradient"
            x1="0%"
            y1="0%"
            x2="100%"
            y2="100%"
          >
            <stop
              offset="0%"
              stop-color="#cc785c"
            />
            <stop
              offset="50%"
              stop-color="#d4a27f"
            />
            <stop
              offset="100%"
              stop-color="#cc785c"
            />
          </linearGradient>
          <filter
            id="adaptive-aether-glow"
            x="-50%"
            y="-50%"
            width="200%"
            height="200%"
          >
            <feGaussianBlur
              stdDeviation="3"
              result="coloredBlur"
            />
            <feMerge>
              <feMergeNode in="coloredBlur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>
      </svg>

      <!-- Ripple layers - start after fill completes -->
      <div
        class="adaptive-ripple r-1"
        :class="{ active: adaptiveFillComplete }"
      >
        <svg
          viewBox="0 0 799.31 752.14"
          class="adaptive-logo-img"
        >
          <path
            :d="adaptiveAetherPath"
            fill="none"
            stroke="url(#adaptive-aether-gradient)"
            stroke-width="2"
            vector-effect="non-scaling-stroke"
          />
        </svg>
      </div>
      <div
        class="adaptive-ripple r-2"
        :class="{ active: adaptiveFillComplete }"
      >
        <svg
          viewBox="0 0 799.31 752.14"
          class="adaptive-logo-img"
        >
          <path
            :d="adaptiveAetherPath"
            fill="none"
            stroke="url(#adaptive-aether-gradient)"
            stroke-width="2"
            vector-effect="non-scaling-stroke"
          />
        </svg>
      </div>
      <div
        class="adaptive-ripple r-3"
        :class="{ active: adaptiveFillComplete }"
      >
        <svg
          viewBox="0 0 799.31 752.14"
          class="adaptive-logo-img"
        >
          <path
            :d="adaptiveAetherPath"
            fill="none"
            stroke="url(#adaptive-aether-gradient)"
            stroke-width="2"
            vector-effect="non-scaling-stroke"
          />
        </svg>
      </div>

      <!-- Phase 1: Stroke outline drawing (SVG overlay) -->
      <svg
        class="adaptive-stroke-overlay"
        :class="{ 'stroke-complete': adaptiveStrokeComplete }"
        viewBox="0 0 799.31 752.14"
        xmlns="http://www.w3.org/2000/svg"
      >
        <path
          class="adaptive-stroke-path"
          :d="adaptiveAetherPath"
          style="stroke: url(#adaptive-aether-gradient); filter: url(#adaptive-aether-glow);"
        />
      </svg>

      <!-- Phase 2: Fill using SVG path -->
      <div
        class="adaptive-fill-layer"
        :class="{ 'fill-active': adaptiveStrokeComplete, 'fill-complete': adaptiveFillComplete, 'breathing': adaptiveFillComplete }"
      >
        <svg
          viewBox="0 0 799.31 752.14"
          class="adaptive-fill-img"
        >
          <path
            :d="adaptiveAetherPath"
            fill="url(#adaptive-aether-gradient)"
            fill-rule="evenodd"
          />
        </svg>
      </div>
    </div>

    <!-- Aether logo: single complex path with ripple effect -->
    <svg
      v-else-if="type === 'aether'"
      :key="`aether-${animationKey}`"
      :viewBox="viewBox"
      class="ripple-logo"
      xmlns="http://www.w3.org/2000/svg"
      :style="{ '--anim-delay': `${animDelay}ms` }"
    >
      <defs>
        <path
          id="aether-path"
          ref="aetherPathRef"
          :d="aetherPath"
        />
        <!-- Gradient for breathing glow effect -->
        <linearGradient
          id="aether-gradient"
          x1="0%"
          y1="0%"
          x2="100%"
          y2="100%"
        >
          <stop
            offset="0%"
            stop-color="#cc785c"
          />
          <stop
            offset="50%"
            stop-color="#d4a27f"
          />
          <stop
            offset="100%"
            stop-color="#cc785c"
          />
        </linearGradient>
        <!-- Glow filter for breathing effect -->
        <filter
          id="aether-glow"
          x="-50%"
          y="-50%"
          width="200%"
          height="200%"
        >
          <feGaussianBlur
            stdDeviation="4"
            result="coloredBlur"
          />
          <feMerge>
            <feMergeNode in="coloredBlur" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>

      <!-- Static mode: just show filled logo with fade-in animation -->
      <template v-if="static">
        <use
          href="#aether-path"
          class="static-fill"
          :style="{ fill: strokeColor }"
        />
      </template>

      <!-- Animated mode -->
      <template v-else>
        <!-- Main logo - with stroke drawing animation -->
        <use
          href="#aether-path"
          class="fine-line stroke-draw aether-stroke"
          :class="{ 'draw-complete': drawComplete, 'breathing': drawComplete && !disableRipple }"
          :style="{ stroke: drawComplete ? 'url(#aether-gradient)' : strokeColor, '--path-length': aetherPathLength, transformOrigin: aetherCenter }"
          :filter="drawComplete ? 'url(#aether-glow)' : 'none'"
        />
        <!-- Main logo - fill (fade in after draw) -->
        <use
          href="#aether-path"
          class="aether-fill"
          :class="{ 'fill-active': drawComplete, 'breathing': drawComplete && !disableRipple }"
          :style="{ fill: strokeColor, transformOrigin: aetherCenter }"
        />
        <use
          v-if="!disableRipple"
          href="#aether-path"
          class="fine-line ripple d-1"
          :class="{ 'ripple-active': drawComplete }"
          :style="{ stroke: strokeColor, transformOrigin: aetherCenter }"
        />
        <use
          v-if="!disableRipple"
          href="#aether-path"
          class="fine-line ripple d-2"
          :class="{ 'ripple-active': drawComplete }"
          :style="{ stroke: strokeColor, transformOrigin: aetherCenter }"
        />
        <use
          v-if="!disableRipple"
          href="#aether-path"
          class="fine-line ripple d-3"
          :class="{ 'ripple-active': drawComplete }"
          :style="{ stroke: strokeColor, transformOrigin: aetherCenter }"
        />
      </template>
    </svg>
  
    <!-- Standard single-path logos -->
    <svg
      v-else
      :key="`${type}-${animationKey}`"
      :viewBox="viewBox"
      class="ripple-logo"
      xmlns="http://www.w3.org/2000/svg"
      :style="{ '--anim-delay': `${animDelay}ms` }"
    >
      <defs>
        <path
          :id="pathId"
          :d="pathData"
        />
        <!-- Gemini multi-layer gradients -->
        <template v-if="type === 'gemini'">
          <!-- Fill gradients -->
          <linearGradient
            :id="`${pathId}-fill-0`"
            gradientUnits="userSpaceOnUse"
            x1="7"
            x2="11"
            y1="15.5"
            y2="12"
          >
            <stop stop-color="#08B962" />
            <stop
              offset="1"
              stop-color="#08B962"
              stop-opacity="0"
            />
          </linearGradient>
          <linearGradient
            :id="`${pathId}-fill-1`"
            gradientUnits="userSpaceOnUse"
            x1="8"
            x2="11.5"
            y1="5.5"
            y2="11"
          >
            <stop stop-color="#F94543" />
            <stop
              offset="1"
              stop-color="#F94543"
              stop-opacity="0"
            />
          </linearGradient>
          <linearGradient
            :id="`${pathId}-fill-2`"
            gradientUnits="userSpaceOnUse"
            x1="3.5"
            x2="17.5"
            y1="13.5"
            y2="12"
          >
            <stop stop-color="#FABC12" />
            <stop
              offset=".46"
              stop-color="#FABC12"
              stop-opacity="0"
            />
          </linearGradient>
          <!-- Stroke gradient for outline - 4 directional gradients to match logo colors -->
          <!-- Top point = red, Right point = blue, Bottom point = green, Left point = yellow -->
          <linearGradient
            :id="`${pathId}-stroke-v`"
            gradientUnits="userSpaceOnUse"
            x1="12"
            x2="12"
            y1="1"
            y2="23"
          >
            <stop
              offset="0%"
              stop-color="#F94543"
            />
            <stop
              offset="50%"
              stop-color="#3186FF"
            />
            <stop
              offset="100%"
              stop-color="#08B962"
            />
          </linearGradient>
          <linearGradient
            :id="`${pathId}-stroke-h`"
            gradientUnits="userSpaceOnUse"
            x1="1"
            x2="23"
            y1="12"
            y2="12"
          >
            <stop
              offset="0%"
              stop-color="#FABC12"
            />
            <stop
              offset="50%"
              stop-color="#3186FF"
            />
            <stop
              offset="100%"
              stop-color="#3186FF"
            />
          </linearGradient>
          <!-- Mask for fill-inward animation (controlled by JS) -->
          <mask :id="`${pathId}-fill-mask`">
            <rect
              x="-4"
              y="-4"
              width="32"
              height="32"
              fill="white"
            />
            <circle
              cx="12"
              cy="12"
              :r="geminiFillRadius"
              fill="black"
            />
          </mask>
        </template>
      </defs>

      <!-- OpenAI special rendering: stroke outline -> fill -> rotate + breathe -->
      <template v-if="type === 'openai'">
        <!-- Outer breathing wrapper (scale pulse) -->
        <g
          class="openai-breathe-group"
          :class="{ 'breathing': drawComplete }"
          :style="{ transformOrigin: transformOrigin }"
        >
          <!-- Inner rotation wrapper -->
          <g
            class="openai-rotate-group"
            :class="{ 'rotating': drawComplete }"
            :style="{ transformOrigin: transformOrigin }"
          >
            <!-- Step 1: Stroke outline drawing -->
            <use
              :href="`#${pathId}`"
              class="openai-outline"
              :class="{ 'outline-complete': drawComplete }"
              stroke="currentColor"
            />
            <!-- Step 2: Fill layer (appears after outline) -->
            <use
              :href="`#${pathId}`"
              class="openai-fill"
              :class="{ 'fill-active': drawComplete }"
              fill="currentColor"
              fill-rule="evenodd"
            />
          </g>
        </g>
      </template>

      <!-- Claude special rendering: stroke outline -> fill -> ripple -->
      <template v-else-if="type === 'claude'">
        <!-- Step 1: Stroke outline drawing -->
        <use
          :href="`#${pathId}`"
          class="claude-outline"
          :class="{ 'outline-complete': drawComplete }"
          stroke="#D97757"
        />
        <!-- Step 2: Fill layer (appears after outline) -->
        <use
          :href="`#${pathId}`"
          class="claude-fill"
          :class="{ 'fill-active': drawComplete }"
          fill="#D97757"
        />
        <!-- Step 3: Ripple waves (after fill complete) -->
        <use
          :href="`#${pathId}`"
          class="fine-line claude-ripple d-1"
          :class="{ 'ripple-active': drawComplete }"
          :style="{ stroke: '#D97757', transformOrigin: transformOrigin }"
        />
        <use
          :href="`#${pathId}`"
          class="fine-line claude-ripple d-2"
          :class="{ 'ripple-active': drawComplete }"
          :style="{ stroke: '#D97757', transformOrigin: transformOrigin }"
        />
        <use
          :href="`#${pathId}`"
          class="fine-line claude-ripple d-3"
          :class="{ 'ripple-active': drawComplete }"
          :style="{ stroke: '#D97757', transformOrigin: transformOrigin }"
        />
      </template>

      <!-- Gemini special rendering: stroke outline -> fill -> breathe -->
      <template v-else-if="type === 'gemini'">
        <!-- Step 1: Stroke outline drawing (multi-layer colorful) -->
        <g
          class="gemini-outline-group"
          :class="{ 'outline-complete': drawComplete }"
        >
          <use
            :href="`#${pathId}`"
            class="gemini-outline"
            stroke="#3186FF"
          />
          <use
            :href="`#${pathId}`"
            class="gemini-outline"
            :style="{ stroke: `url(#${pathId}-fill-0)` }"
          />
          <use
            :href="`#${pathId}`"
            class="gemini-outline"
            :style="{ stroke: `url(#${pathId}-fill-1)` }"
          />
          <use
            :href="`#${pathId}`"
            class="gemini-outline"
            :style="{ stroke: `url(#${pathId}-fill-2)` }"
          />
        </g>
        <!-- Step 2: Fill layer (appears after outline, with inward fill animation) -->
        <g
          class="gemini-fill"
          :class="{ 'fill-complete': fillComplete }"
          :mask="`url(#${pathId}-fill-mask)`"
        >
          <use
            :href="`#${pathId}`"
            fill="#3186FF"
          />
          <use
            :href="`#${pathId}`"
            :fill="`url(#${pathId}-fill-0)`"
          />
          <use
            :href="`#${pathId}`"
            :fill="`url(#${pathId}-fill-1)`"
          />
          <use
            :href="`#${pathId}`"
            :fill="`url(#${pathId}-fill-2)`"
          />
        </g>
        <!-- Step 3: Ripple waves (after fill complete) -->
        <g v-if="!disableRipple">
          <g
            class="gemini-ripple d-1"
            :class="{ 'ripple-active': fillComplete }"
            :style="{ transformOrigin: transformOrigin }"
          >
            <use
              :href="`#${pathId}`"
              fill="#3186FF"
            />
            <use
              :href="`#${pathId}`"
              :fill="`url(#${pathId}-fill-0)`"
            />
            <use
              :href="`#${pathId}`"
              :fill="`url(#${pathId}-fill-1)`"
            />
            <use
              :href="`#${pathId}`"
              :fill="`url(#${pathId}-fill-2)`"
            />
          </g>
          <g
            class="gemini-ripple d-2"
            :class="{ 'ripple-active': fillComplete }"
            :style="{ transformOrigin: transformOrigin }"
          >
            <use
              :href="`#${pathId}`"
              fill="#3186FF"
            />
            <use
              :href="`#${pathId}`"
              :fill="`url(#${pathId}-fill-0)`"
            />
            <use
              :href="`#${pathId}`"
              :fill="`url(#${pathId}-fill-1)`"
            />
            <use
              :href="`#${pathId}`"
              :fill="`url(#${pathId}-fill-2)`"
            />
          </g>
          <g
            class="gemini-ripple d-3"
            :class="{ 'ripple-active': fillComplete }"
            :style="{ transformOrigin: transformOrigin }"
          >
            <use
              :href="`#${pathId}`"
              fill="#3186FF"
            />
            <use
              :href="`#${pathId}`"
              :fill="`url(#${pathId}-fill-0)`"
            />
            <use
              :href="`#${pathId}`"
              :fill="`url(#${pathId}-fill-1)`"
            />
            <use
              :href="`#${pathId}`"
              :fill="`url(#${pathId}-fill-2)`"
            />
          </g>
        </g>
      </template>

      <!-- Other logos: stroke-based rendering -->
      <template v-else>
        <!-- Static center icon with stroke drawing animation -->
        <use
          :href="`#${pathId}`"
          class="fine-line stroke-draw"
          :class="{ 'draw-complete': drawComplete }"
          :style="{ stroke: strokeColor, '--path-length': pathLength }"
        />

        <!-- Ripple waves - only active after drawing completes -->
        <g>
          <use
            :href="`#${pathId}`"
            class="fine-line ripple d-1"
            :class="{ 'ripple-active': drawComplete }"
            :style="{ stroke: strokeColor, transformOrigin: transformOrigin }"
          />
          <use
            :href="`#${pathId}`"
            class="fine-line ripple d-2"
            :class="{ 'ripple-active': drawComplete }"
            :style="{ stroke: strokeColor, transformOrigin: transformOrigin }"
          />
          <use
            :href="`#${pathId}`"
            class="fine-line ripple d-3"
            :class="{ 'ripple-active': drawComplete }"
            :style="{ stroke: strokeColor, transformOrigin: transformOrigin }"
          />
        </g>
      </template>
    </svg>
  </Transition>
</template>


<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { AETHER_LOGO_CENTER, AETHER_LOGO_PATH, AETHER_LOGO_VIEWBOX } from '@/constants/logoPaths'

type LogoType = 'aether' | 'claude' | 'openai' | 'gemini'

const props = withDefaults(
  defineProps<{
    type: LogoType
    size?: number
    /** Use adaptive SVG image instead of inline SVG (for aether type) */
    useAdaptive?: boolean
    /** Disable ripple animation */
    disableRipple?: boolean
    /** Delay before animation starts (ms) */
    animDelay?: number
    /** Static mode - no animations at all, just show the filled logo */
    static?: boolean
  }>(),
  {
    size: 200,
    useAdaptive: false,
    disableRipple: false,
    animDelay: 0,
    static: false
  }
)

// Drawing animation state
const drawComplete = ref(false)
const fillComplete = ref(false)
const geminiFillRadius = ref(15) // SVG mask circle radius for fill animation
const animationKey = ref(0) // Force re-render to restart CSS animations
const drawDuration = 1000 // ms for stroke drawing animation
const openaiOutlineDuration = 1000 // ms - matches CSS animation duration (1.2s)
const geminiOutlineDuration = 900 // ms - matches CSS animation duration (1.8s)

// Timer refs for cleanup on type switch
const animationTimers = ref<number[]>([])
const fillAnimationId = ref<number | null>(null)

// Adaptive aether animation states (3-phase: stroke -> fill -> ripple)
const adaptiveStrokeComplete = ref(false)
const adaptiveFillComplete = ref(false)
const adaptiveStrokeDuration = 1500 // ms for stroke drawing
const adaptiveFillDuration = 600 // ms for fill fade-in (shorter for smoother transition)
const aetherPath = AETHER_LOGO_PATH
const aetherCenter = `${AETHER_LOGO_CENTER.x}px ${AETHER_LOGO_CENTER.y}px`

// Path data from aether_adaptive.svg (viewBox: 0 0 799.31 752.14)
const adaptiveAetherPath = 'M569.100 368.700Q569.900 371.900 570.200 378.100Q571.100 396.300 568.300 413.400C565.100 433.000 558.700 452.800 548.800 470.200Q542.500 481.100 539.100 486.100Q527.900 502.600 513.100 516.300Q499.300 529.000 480.800 539.600C463.100 549.800 445.200 557.800 425.700 563.800Q410.400 568.400 405.600 569.500C390.300 573.100 370.600 576.200 352.900 580.400Q338.100 584.000 330.500 586.200C306.900 592.900 283.100 604.600 264.000 618.500C248.800 629.500 235.500 643.500 223.600 658.300Q212.700 671.900 203.500 690.300Q191.400 714.300 188.700 720.800Q184.400 731.100 176.500 750.800A0.600 0.600 0 0 0 177.000 751.600L198.800 751.600A2.400 2.400 9.8 0 0 201.100 750.000C210.500 723.800 222.200 698.700 238.300 676.100Q250.400 659.200 263.000 647.800Q283.300 629.300 308.900 617.200Q324.400 609.900 349.200 602.700Q357.300 600.400 381.000 596.000Q404.200 591.600 412.800 589.500Q425.100 586.400 443.400 580.300Q458.800 575.100 473.000 568.000Q491.000 559.000 505.500 549.500A0.400 0.300 62.9 0 1 506.100 549.600Q506.900 551.800 507.800 553.500Q515.800 569.100 546.000 626.000Q554.400 641.700 555.400 643.700Q562.300 657.300 564.600 661.500C569.700 670.600 573.900 679.800 578.900 688.500Q586.000 700.800 588.100 704.400C597.100 719.200 613.800 733.700 630.300 741.200Q634.300 743.100 646.000 746.700Q652.200 748.600 659.400 749.300Q671.600 750.600 673.100 750.600Q691.500 751.600 725.100 751.600Q751.900 751.600 794.900 751.400Q796.800 751.400 798.600 748.600A1.200 1.200 0 0 0 798.800 747.700Q797.900 744.400 795.800 740.200Q780.100 708.200 769.300 687.400Q766.700 682.500 765.700 680.600Q754.700 658.800 739.000 628.800C726.200 604.300 717.600 587.300 705.800 565.100Q701.200 556.400 691.100 536.900Q683.000 521.000 671.400 499.900C666.400 490.800 656.800 470.600 648.500 456.000C640.200 441.400 631.800 423.800 625.100 411.400Q616.100 395.000 609.600 382.100C600.700 364.400 591.200 347.100 581.800 329.000Q562.500 292.000 552.500 273.200C546.700 262.300 540.300 249.200 535.500 240.200Q520.300 211.500 506.000 183.800C495.700 163.800 483.600 141.800 473.700 122.500C466.700 108.900 458.500 94.200 451.900 81.300Q445.100 67.900 435.600 50.400Q426.400 33.500 414.700 10.400C412.000 5.100 408.600 0.600 402.500 0.500C397.900 0.400 394.300 2.900 392.100 6.900Q386.700 16.300 378.700 31.500Q360.600 65.900 349.200 86.700Q341.500 100.800 334.600 114.500Q331.300 121.200 319.900 142.200Q315.600 150.000 308.200 164.300Q305.300 169.900 296.500 186.300C285.400 206.800 274.800 227.400 264.900 245.700C251.400 270.800 237.100 298.400 225.100 320.900C216.600 336.800 207.700 354.700 199.400 370.000C186.800 393.100 175.400 415.600 161.900 440.600C155.200 453.000 145.300 472.900 137.100 487.600Q129.400 501.400 121.200 517.900Q116.500 527.300 85.500 585.600Q67.700 619.000 64.800 624.900C57.700 638.900 47.100 658.100 40.500 671.200Q25.700 700.300 11.600 727.600Q7.200 736.300 0.600 750.500A0.800 0.800 12.4 0 0 1.300 751.600L23.900 751.600A1.400 1.400 -77.4 0 0 25.200 750.800Q40.000 719.200 51.100 698.700Q52.000 697.100 67.600 667.200Q84.900 634.200 103.200 608.200Q123.000 580.200 152.700 554.100C157.700 549.700 163.500 545.700 169.200 541.200Q175.100 536.500 182.000 532.000Q222.200 505.400 266.800 491.800Q277.100 488.700 293.800 484.100Q302.500 481.800 309.400 480.600C334.800 476.000 358.000 471.700 381.400 465.400Q392.800 462.300 399.400 459.700Q415.400 453.400 432.400 444.600Q443.500 438.800 451.600 433.000C465.000 423.400 476.400 412.300 487.200 399.300C503.500 379.800 515.500 355.900 522.500 331.500A0.200 0.200 0 0 1 522.800 331.500Q523.200 332.700 522.900 334.400Q520.700 349.300 515.900 364.900Q511.000 380.600 503.900 393.800C486.600 425.800 457.300 453.500 424.000 470.000Q403.300 480.200 380.600 487.100Q369.500 490.500 350.900 494.200Q309.900 502.300 300.400 504.800C252.800 517.300 207.600 540.700 172.000 574.400Q160.300 585.400 155.000 590.800Q149.100 597.000 137.700 611.400C130.400 620.600 125.200 628.800 117.900 639.600Q111.300 649.300 104.800 661.500C88.800 691.300 74.600 719.700 60.400 750.600A0.700 0.700 0 0 0 61.000 751.600L84.000 751.600A1.000 0.900 14.4 0 0 84.800 751.100Q86.200 748.500 89.100 742.100Q100.600 716.900 117.200 684.500C124.600 669.900 132.800 656.100 141.500 643.000C152.500 626.600 165.000 612.000 176.200 600.200Q193.300 582.200 217.100 565.400Q233.500 553.800 253.000 544.500Q278.800 532.400 303.600 525.600Q312.900 523.000 334.800 518.900Q363.800 513.400 386.500 507.200Q419.800 498.200 450.700 479.200Q488.100 456.300 513.000 419.300C529.900 394.200 540.200 364.300 544.200 334.400Q544.700 331.100 544.600 319.700A0.200 0.200 0 0 1 544.900 319.500L545.300 319.500A0.100 0.100 0 0 1 545.400 319.600Q551.700 350.700 545.400 381.400C543.200 392.000 539.700 405.500 534.500 416.700Q528.300 430.000 527.300 431.600Q513.400 453.800 500.000 467.700C484.800 483.500 465.800 497.400 445.100 507.600Q431.900 514.200 428.300 515.600Q409.000 523.200 389.700 528.500C374.400 532.600 357.700 534.800 342.000 538.000Q331.300 540.200 312.200 545.400Q275.000 555.500 242.500 575.700Q223.600 587.400 208.200 601.800Q191.800 617.100 179.300 633.800Q159.000 661.000 140.900 698.900Q135.600 709.900 123.100 736.900C120.800 741.900 119.200 746.300 117.100 750.700A0.700 0.700 0 0 0 117.700 751.600L139.800 751.600A1.300 1.300 0 0 0 140.800 751.100Q142.100 749.500 142.500 748.400Q145.900 739.800 149.400 732.200Q157.900 714.000 168.200 693.400Q177.600 674.600 184.600 664.000Q194.700 648.700 204.700 636.900Q236.300 599.400 281.300 579.100C299.000 571.100 319.700 563.700 339.400 559.900C365.500 555.000 389.900 550.700 413.500 543.300C437.400 535.800 461.000 525.000 481.700 510.600C492.400 503.200 503.000 495.400 511.400 486.900C523.200 475.000 535.100 460.500 543.600 445.400Q549.300 435.300 551.100 431.600Q563.200 406.600 567.400 378.500Q568.600 370.400 568.900 368.700A0.100 0.100 0 0 1 569.100 368.700 M507.900 291.500C509.300 280.500 508.500 268.100 507.400 257.800Q506.700 251.800 506.100 249.000C504.400 241.300 503.600 234.600 501.200 227.300Q496.500 213.100 493.500 207.300Q471.200 164.400 471.000 164.100C462.200 146.800 452.100 129.700 443.400 112.100C439.900 104.900 432.600 90.900 426.800 80.800C422.600 73.500 419.400 66.200 415.200 58.500Q409.100 47.700 409.000 47.500C408.100 45.500 407.700 44.100 407.000 42.600A0.300 0.300 0 0 0 406.400 42.700Q400.600 75.200 412.000 106.200Q418.500 124.000 423.100 132.300Q438.500 160.200 446.900 176.900C455.600 194.000 465.900 212.000 473.900 228.000C481.200 242.400 491.100 259.400 498.300 274.200Q502.700 283.500 507.300 291.600A0.300 0.300 0 0 0 507.900 291.500 M383.500 78.700C380.900 85.500 379.900 91.900 378.500 100.000C373.800 127.500 381.000 155.200 393.900 179.100Q399.200 188.900 412.300 214.000C425.300 238.800 431.400 249.500 443.700 273.000Q449.200 283.400 457.100 297.900Q469.000 319.700 470.500 322.800Q479.600 340.900 486.800 353.500Q487.500 354.700 488.500 357.400A0.400 0.400 0 0 0 489.300 357.400Q496.100 343.900 501.600 329.100A2.100 2.100 52.5 0 0 501.800 328.200Q501.500 325.400 500.700 323.900Q491.200 306.400 483.200 291.000Q464.900 256.200 450.800 229.800Q449.800 228.000 446.400 221.800C434.200 199.100 422.000 175.600 409.900 153.700C399.100 134.200 389.800 115.500 386.500 95.900Q385.300 89.400 383.900 78.800A0.200 0.200 0 0 0 383.500 78.700 M355.600 127.900Q345.300 163.000 354.600 199.500Q359.900 220.200 370.900 241.400Q389.600 277.300 422.000 337.300Q441.100 372.800 452.700 395.000Q453.600 396.800 456.500 400.700A0.500 0.500 0 0 0 457.200 400.700C463.900 393.900 470.600 387.000 475.600 379.600A1.100 1.100 54.1 0 0 475.800 378.600Q475.500 377.700 473.600 374.200Q458.600 346.900 451.600 333.500C434.000 299.900 414.900 265.000 397.100 231.400Q388.000 214.000 372.100 183.900Q358.300 157.700 356.200 128.000A0.300 0.300 0 0 0 355.600 127.900 M133.700 541.900A0.500 0.500 0 0 0 134.400 542.400C145.400 532.300 157.300 522.500 170.300 514.400Q173.900 512.200 184.800 504.900A6.700 6.500 -88.8 0 0 186.600 503.200Q189.100 499.700 191.400 495.100Q202.200 474.100 212.900 453.800C217.300 445.500 220.600 439.900 223.600 433.900Q227.700 425.400 231.200 418.700Q239.200 403.300 241.800 398.500C252.100 379.600 261.800 360.100 273.600 338.400Q280.000 326.500 288.300 310.600C297.500 292.900 308.200 273.700 318.100 254.300C324.100 242.600 331.000 230.600 336.800 219.400A2.800 2.800 -39.3 0 0 337.000 217.400Q331.000 196.200 328.800 172.900A0.400 0.400 0 0 0 328.000 172.800Q326.900 174.900 326.700 175.300Q321.200 185.400 309.400 208.200Q303.900 218.900 300.300 225.600C290.100 244.500 281.400 261.400 268.400 285.600Q257.400 306.200 245.700 328.400Q203.600 407.900 187.300 438.400C185.000 442.900 183.000 447.300 180.400 452.100Q159.900 490.000 139.500 530.300C137.600 534.100 134.500 537.500 133.700 541.900 M348.300 244.700A0.300 0.300 0 0 0 347.700 244.700L220.200 485.500A0.300 0.300 0 0 0 220.600 486.000L256.400 472.100A0.300 0.300 0 0 0 256.600 471.900L362.500 271.900A0.300 0.300 0 0 0 362.500 271.500L348.300 244.700 M374.800 295.800A0.300 0.300 0 0 0 374.2 295.9L285.3 463.8A0.3 0.3 0 0 0 285.6 464.3L319.7 457.0A0.3 0.3 0 0 0 320.0 456.8L390.2 324.1A0.3 0.3 0 0 0 390.2 323.8L374.8 295.8 M401.7 347.4Q393.7 363.6 389.3 371.6C378.0 392.3 364.8 418.2 356.8 433.1Q350.8 444.3 346.8 451.0A0.4 0.4 -79.4 0 0 347.2 451.6Q363.0 449.0 377.8 444.6Q410.1 434.9 438.8 416.4A1.3 1.3 -34.8 0 0 439.1 414.5Q437.2 411.9 436.0 409.7Q422.3 383.5 402.4 347.4A0.4 0.4 44 0 0 401.7 347.4 M591.7 397.1Q590.1 421.4 584.1 443.0A4.9 4.9 38.7 0 0 584.5 446.6Q604.0 482.7 615.1 503.8C619.2 511.6 622.6 517.4 626.0 524.4Q629.4 531.3 633.9 539.9C639.7 550.9 642.4 556.5 647.4 565.1Q650.9 571.1 658.0 585.6C665.2 600.2 679.2 625.2 688.2 643.5C697.0 661.3 704.0 675.2 714.2 689.8Q723.6 703.1 732.0 710.1Q746.6 722.4 764.6 728.0Q764.8 728.0 765.1 727.9A0.5 0.5 -31.1 0 0 765.3 727.2Q758.7 716.1 751.5 701.3Q744.6 687.1 738.1 675.0C728.5 657.2 719.8 639.3 710.6 622.1Q701.3 604.6 690.4 583.0C682.9 568.3 675.1 554.1 667.8 540.0Q659.6 523.9 651.0 507.7Q637.2 481.8 623.3 455.4Q612.0 434.0 599.9 410.8Q596.1 403.8 592.5 397.0A0.4 0.4 -57.1 0 0 591.7 397.1 M573.1 471.2A0.4 0.4 0 0 0 572.5 471.3Q566.3 483.3 561.3 491.7C560.3 493.4 559.1 494.7 558.1 496.0A2.8 2.8 0 0 0 558.0 499.0Q576.2 532.2 586.7 552.7Q591.2 561.4 593.9 566.2Q598.6 574.6 609.0 594.7Q618.7 613.4 624.0 623.4Q630.6 635.7 646.7 668.1Q655.5 685.8 667.2 698.9Q687.9 722.1 718.8 727.5A0.4 0.4 0 0 0 719.1 726.8Q711.9 720.8 704.4 712.5Q698.9 706.5 693.6 698.2Q685.8 685.9 684.2 682.8Q674.6 664.6 666.8 649.5Q614.4 548.1 576.9 477.7Q574.8 473.8 573.1 471.2 M667.5 728.3A0.3 0.3 0 0 0 667.7 727.7Q663.2 724.7 658.3 720.4Q653.6 716.1 650.0 711.9C634.2 693.5 625.2 674.4 613.4 650.7Q608.8 641.6 602.2 629.1C593.8 613.4 586.3 598.2 580.5 587.2C571.2 569.5 558.3 546.0 550.7 530.6Q547.6 524.3 543.3 517.4A0.9 0.9 0 0 0 541.9 517.3C535.1 524.7 530.2 530.1 523.8 535.2A0.8 0.7 59.4 0 0 523.6 536.1Q525.0 539.6 525.2 539.8C529.3 547.2 533.0 554.7 536.6 561.2Q546.6 579.1 554.6 595.3Q561.7 609.9 569.1 622.8Q575.4 634.1 585.9 655.0C591.9 667.1 598.4 678.7 605.5 690.5Q610.6 698.8 616.3 705.5C628.8 720.0 648.6 727.7 667.5 728.3'
const aetherPathRef = ref<SVGPathElement | null>(null)
const aetherPathLength = ref(12000)

const updateAetherPathLength = () => {
  if (props.type !== 'aether') return
  if (!aetherPathRef.value) return

  try {
    aetherPathLength.value = aetherPathRef.value.getTotalLength()
  } catch {
    // keep the fallback length to avoid animation jitter
  }
}

// Animate fill from edges to center (Gemini)
const geminiFillDuration = 600 // ms - shorter fill animation
const animateFill = () => {
  const startRadius = 15
  const endRadius = 0
  const startTime = performance.now()

  const animate = (currentTime: number) => {
    const elapsed = currentTime - startTime
    const progress = Math.min(elapsed / geminiFillDuration, 1)
    // Ease-out curve: fast start, slow end - fill appears quickly
    const easedProgress = 1 - Math.pow(1 - progress, 2)
    geminiFillRadius.value = startRadius - (startRadius - endRadius) * easedProgress

    if (progress < 1) {
      fillAnimationId.value = requestAnimationFrame(animate)
    } else {
      fillAnimationId.value = null
      fillComplete.value = true
      // Outline fades out after fill completes
      drawComplete.value = true
    }
  }

  fillAnimationId.value = requestAnimationFrame(animate)
}

// Clear all pending animation timers
// Clear all pending animation timers
const clearAnimationTimers = () => {
  animationTimers.value.forEach((timerId) => {
    clearTimeout(timerId)
  })
  animationTimers.value = []
  
  if (fillAnimationId.value !== null) {
    cancelAnimationFrame(fillAnimationId.value)
    fillAnimationId.value = null
  }
}

// Helper to add a tracked timer
const addTimer = (callback: () => void, delay: number) => {
  const timerId = window.setTimeout(callback, delay)
  animationTimers.value.push(timerId)
  return timerId
}

// Start animation sequence
const startAnimation = () => {
  // Static mode: skip all animations, show filled logo immediately
  if (props.static) {
    drawComplete.value = true
    fillComplete.value = true
    adaptiveStrokeComplete.value = true
    adaptiveFillComplete.value = true
    return
  }

  // Global delay before starting any animation logic
  addTimer(() => {
    if (props.type === 'aether' && props.useAdaptive) {
      // For adaptive Aether: 3-phase animation
      // Phase 1: Stroke drawing (CSS animation handles this)
      addTimer(() => {
        // Phase 2: Fill fades in after stroke completes
        adaptiveStrokeComplete.value = true

        // Phase 3: Ripples start slightly before fill completes for smoother transition
        // Start ripple at 70% of fill duration to overlap and avoid the "pause" feeling
        addTimer(() => {
          adaptiveFillComplete.value = true
        }, adaptiveFillDuration * 0.7)
      }, adaptiveStrokeDuration)
    } else if (props.type === 'gemini') {
      // For Gemini: start fill right when outline completes (ease-in curve ends fast)
      addTimer(() => {
        animateFill()
      }, geminiOutlineDuration)
    } else if (props.type === 'openai') {
      // For OpenAI: start fill right when outline completes
      addTimer(() => {
        drawComplete.value = true
      }, openaiOutlineDuration)
    } else if (props.type === 'claude') {
      // For Claude: start fill right when outline completes
      addTimer(() => {
        drawComplete.value = true
      }, drawDuration)
    } else {
      // For other logos: set drawComplete after stroke animation
      addTimer(() => {
        drawComplete.value = true
      }, drawDuration)
    }
  }, props.animDelay)
}

// Reset and restart animation when type changes
watch(
  () => props.type,
  async (_newType, oldType) => {
    // Clear any pending timers from previous logo type
    clearAnimationTimers()

    // Reset all animation states immediately
    drawComplete.value = false
    fillComplete.value = false
    geminiFillRadius.value = 15
    adaptiveStrokeComplete.value = false
    adaptiveFillComplete.value = false

    // Only increment key if type actually changed (not on initial render)
    // Increment synchronously to avoid frame delay
    if (oldType !== undefined) {
      animationKey.value++
    }

    // Wait for DOM update
    await nextTick()
    // Small delay to allow CSS transition to start
    await new Promise(resolve => setTimeout(resolve, 30))
    updateAetherPathLength()
    startAnimation()
  }
)

// Start animation on mount
onMounted(() => {
  updateAetherPathLength()
  startAnimation()
})

// Clean up timers on unmount
onUnmounted(() => {
  clearAnimationTimers()
})

// Expose animation states for parent component
defineExpose({
  fillComplete,
  drawComplete
})

const pathId = computed(() => `${props.type}-ripple-path`)

// Different viewBox for each logo to center them properly
// Aether logo viewBox: original is "419 249 954 933", center around (896, 715)
// Smaller viewBox = larger icon appearance
const viewBox = computed(() => {
  const viewBoxes: Record<LogoType, string> = {
    aether: AETHER_LOGO_VIEWBOX, // Original Aether logo viewBox
    claude: '0 0 24 24', // Original Claude viewBox
    openai: '0 0 24 24', // Original OpenAI viewBox
    gemini: '-4 -4 32 32'
  }
  return viewBoxes[props.type]
})

// Path lengths for different logos (approximate values for stroke animation)
const pathLength = computed(() => {
  if (props.type === 'aether') {
    return aetherPathLength.value
  }

  const lengths: Record<Exclude<LogoType, 'aether'>, number> = {
    claude: 300,
    openai: 200,
    gemini: 150
  }
  return lengths[props.type]
})

const pathData = computed(() => {
  if (props.type === 'aether') {
    return aetherPath
  }

  const paths: Record<Exclude<LogoType, 'aether'>, string> = {
    claude:
      'M4.709 15.955l4.72-2.647.08-.23-.08-.128H9.2l-.79-.048-2.698-.073-2.339-.097-2.266-.122-.571-.121L0 11.784l.055-.352.48-.321.686.06 1.52.103 2.278.158 1.652.097 2.449.255h.389l.055-.157-.134-.098-.103-.097-2.358-1.596-2.552-1.688-1.336-.972-.724-.491-.364-.462-.158-1.008.656-.722.881.06.225.061.893.686 1.908 1.476 2.491 1.833.365.304.145-.103.019-.073-.164-.274-1.355-2.446-1.446-2.49-.644-1.032-.17-.619a2.97 2.97 0 01-.104-.729L6.283.134 6.696 0l.996.134.42.364.62 1.414 1.002 2.229 1.555 3.03.456.898.243.832.091.255h.158V9.01l.128-1.706.237-2.095.23-2.695.08-.76.376-.91.747-.492.584.28.48.685-.067.444-.286 1.851-.559 2.903-.364 1.942h.212l.243-.242.985-1.306 1.652-2.064.73-.82.85-.904.547-.431h1.033l.76 1.129-.34 1.166-1.064 1.347-.881 1.142-1.264 1.7-.79 1.36.073.11.188-.02 2.856-.606 1.543-.28 1.841-.315.833.388.091.395-.328.807-1.969.486-2.309.462-3.439.813-.042.03.049.061 1.549.146.662.036h1.622l3.02.225.79.522.474.638-.079.485-1.215.62-1.64-.389-3.829-.91-1.312-.329h-.182v.11l1.093 1.068 2.006 1.81 2.509 2.33.127.578-.322.455-.34-.049-2.205-1.657-.851-.747-1.926-1.62h-.128v.17l.444.649 2.345 3.521.122 1.08-.17.353-.608.213-.668-.122-1.374-1.925-1.415-2.167-1.143-1.943-.14.08-.674 7.254-.316.37-.729.28-.607-.461-.322-.747.322-1.476.389-1.924.315-1.53.286-1.9.17-.632-.012-.042-.14.018-1.434 1.967-2.18 2.945-1.726 1.845-.414.164-.717-.37.067-.662.401-.589 2.388-3.036 1.44-1.882.93-1.086-.006-.158h-.055L4.132 18.56l-1.13.146-.487-.456.061-.746.231-.243 1.908-1.312-.006.006z',
    openai:
      'M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z',
    gemini:
      'M20.616 10.835a14.147 14.147 0 01-4.45-3.001 14.111 14.111 0 01-3.678-6.452.503.503 0 00-.975 0 14.134 14.134 0 01-3.679 6.452 14.155 14.155 0 01-4.45 3.001c-.65.28-1.318.505-2.002.678a.502.502 0 000 .975c.684.172 1.35.397 2.002.677a14.147 14.147 0 014.45 3.001 14.112 14.112 0 013.679 6.453.502.502 0 00.975 0c.172-.685.397-1.351.677-2.003a14.145 14.145 0 013.001-4.45 14.113 14.113 0 016.453-3.678.503.503 0 000-.975 13.245 13.245 0 01-2.003-.678z'
  }
  return paths[props.type]
})

const strokeColor = computed(() => {
  if (props.type === 'aether' || props.type === 'openai') {
    return 'currentColor'
  }

  if (props.type === 'claude') {
    return '#D97757'
  }

  if (props.type === 'gemini') {
    return `url(#${pathId.value}-gradient)`
  }

  return 'currentColor'
})

// Each logo has different center point based on their path coordinates
const transformOrigin = computed(() => {
  if (props.type === 'aether') {
    return aetherCenter
  }

  // Claude logo center - the sunburst visual center is around (11, 10)
  if (props.type === 'claude') {
    return '12.6px 12.7px'
  }

  return '12px 12px'
})
</script>

<style scoped>
.ripple-logo-container {
  display: flex;
  align-items: center;
  justify-content: center;
}

.ripple-logo {
  width: 100%;
  height: 100%;
  overflow: visible;
}

.fine-line {
  fill: none;
  stroke-width: 0.6px;
  vector-effect: non-scaling-stroke;
}

/* Stroke drawing animation - handwriting effect */
@keyframes stroke-draw {
  0% {
    stroke-dashoffset: var(--path-length);
    opacity: 0.3;
  }
  10% {
    opacity: 1;
  }
  100% {
    stroke-dashoffset: 0;
    opacity: 1;
  }
}

.stroke-draw {
  stroke-dasharray: var(--path-length);
  stroke-dashoffset: var(--path-length);
  animation: stroke-draw 1.2s cubic-bezier(0.4, 0, 0.2, 1) forwards;
  animation-delay: var(--anim-delay, 0s);
}

.stroke-draw.delay-1 {
  animation-delay: 0.3s;
}

.stroke-draw.draw-complete {
  stroke-dasharray: none;
  stroke-dashoffset: 0;
  animation: none;
}

/* Ripple breathing animation - multiple directions for variety */
@keyframes ripple-expand {
  0% {
    transform: scale(1);
    opacity: 0.5;
  }
  100% {
    transform: scale(2.5);
    opacity: 0;
  }
}

@keyframes ripple-expand-up {
  0% {
    transform: scale(1) translateY(0);
    opacity: 0.5;
  }
  100% {
    transform: scale(2) translateY(-30%);
    opacity: 0;
  }
}

@keyframes ripple-expand-diagonal {
  0% {
    transform: scale(1) translate(0, 0);
    opacity: 0.5;
  }
  100% {
    transform: scale(2.2) translate(15%, -15%);
    opacity: 0;
  }
}

@keyframes ripple-pulse {
  0% {
    transform: scale(1);
    opacity: 0.4;
  }
  50% {
    transform: scale(1.8);
    opacity: 0.2;
  }
  100% {
    transform: scale(2.5);
    opacity: 0;
  }
}

/* Ripple waves - hidden by default, only show after drawing completes */
.ripple {
  opacity: 0;
  pointer-events: none;
  will-change: transform, opacity;
  transform: translateZ(0); /* GPU acceleration */
}

.ripple.ripple-active {
  animation: ripple-expand 4s cubic-bezier(0, 0, 0.2, 1) infinite;
}

.ripple.ripple-active.d-1 {
  animation-name: ripple-expand;
  animation-delay: 0s;
}
.ripple.ripple-active.d-2 {
  animation-name: ripple-expand-up;
  animation-delay: 1.3s;
}
.ripple.ripple-active.d-3 {
  animation-name: ripple-expand-diagonal;
  animation-delay: 2.6s;
}

/* OpenAI specific styles - 3 phase animation with smooth transitions */
/* Phase 1: Stroke outline drawing -> Phase 2: Fill fade in -> Phase 3: Rotate + Breathe */

/* Phase 1: Stroke outline drawing - clear and visible */
.openai-outline {
  fill: none;
  stroke-width: 0.5px;
  vector-effect: non-scaling-stroke;
  stroke-dasharray: 200;
  stroke-dashoffset: 200;
  animation: openai-outline-draw 0.9s ease-in forwards;
  animation-delay: var(--anim-delay, 0s);
}

@keyframes openai-outline-draw {
  0% {
    stroke-dashoffset: 200;
    opacity: 0.3;
  }
  10% {
    opacity: 1;
  }
  100% {
    stroke-dashoffset: 0;
    opacity: 1;
  }
}

/* Outline fades out immediately when fill starts */
.openai-outline.outline-complete {
  opacity: 0;
  transition: opacity 0.3s ease-out;
}

/* Phase 2: Fill appears immediately */
.openai-fill {
  opacity: 0;
  visibility: hidden;
}

.openai-fill.fill-active {
  visibility: visible;
  animation: openai-fill-reveal 0.4s ease-out forwards;
}

@keyframes openai-fill-reveal {
  0% {
    opacity: 0;
  }
  100% {
    opacity: 1;
  }
}

/* Phase 3: Rotation + Breathing effects */

@keyframes openai-rotate {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.openai-rotate-group.rotating {
  animation: openai-rotate 25s linear infinite;
}

@keyframes openai-breathe {
  0%, 100% {
    transform: scale(1);
    filter: brightness(1);
  }
  50% {
    transform: scale(1.03);
    filter: brightness(1.05);
  }
}

.openai-breathe-group.breathing {
  animation: openai-breathe 3.5s ease-in-out infinite;
}

/* Claude specific styles - 2 phase animation: outline -> fill -> breathe */

/* Phase 1: Stroke outline drawing */
.claude-outline {
  fill: none;
  stroke-width: 0.15px;
  vector-effect: non-scaling-stroke;
  stroke-dasharray: 300;
  stroke-dashoffset: 300;
  animation: claude-outline-draw 1.2s ease-in forwards;
}

@keyframes claude-outline-draw {
  0% {
    stroke-dashoffset: 300;
    opacity: 0.3;
  }
  10% {
    opacity: 1;
  }
  100% {
    stroke-dashoffset: 0;
    opacity: 1;
  }
}

/* Outline fades out after fill appears */
.claude-outline.outline-complete {
  opacity: 0;
  transition: opacity 0.3s ease-out;
}

/* Phase 2: Fill appears after outline */
.claude-fill {
  opacity: 0;
  visibility: hidden;
}

.claude-fill.fill-active {
  visibility: visible;
  opacity: 1;
  transition: opacity 0.5s ease-in;
}

/* Phase 3: Ripple waves */
.claude-ripple {
  opacity: 0;
  pointer-events: none;
  stroke-width: 0.3px;
}

.claude-ripple.ripple-active {
  animation: claude-ripple-expand 4s cubic-bezier(0, 0, 0.2, 1) infinite;
}

/* Claude ripple - expand from center only */
@keyframes claude-ripple-expand {
  0% {
    transform: scale(1);
    opacity: 0.5;
  }
  100% {
    transform: scale(2.5);
    opacity: 0;
  }
}

.claude-ripple.ripple-active.d-1 {
  animation-delay: 0s;
}
.claude-ripple.ripple-active.d-2 {
  animation-delay: 1.3s;
}
.claude-ripple.ripple-active.d-3 {
  animation-delay: 2.6s;
}

/* Gemini specific styles - 3 phase animation: outline -> fill -> breathe */

/* Phase 1: Stroke outline drawing (multi-layer colorful) */
.gemini-outline-group {
  opacity: 1;
}

/* Outline stays visible, fades out only after fill completes */
.gemini-outline-group.outline-complete {
  opacity: 0;
  transition: opacity 0.3s ease-out;
}

.gemini-outline {
  fill: none;
  stroke-width: 1px;
  vector-effect: non-scaling-stroke;
  stroke-dasharray: 100;
  stroke-dashoffset: 100;
  animation: gemini-outline-draw 1.8s cubic-bezier(0.4, 0, 0.2, 1) forwards;
}

@keyframes gemini-outline-draw {
  0% {
    stroke-dashoffset: 100;
    opacity: 0;
  }
  5% {
    opacity: 0.5;
  }
  15% {
    opacity: 1;
  }
  100% {
    stroke-dashoffset: 0;
    opacity: 1;
  }
}

/* Phase 2: Fill from edges inward to center (uses SVG mask animation) */
.gemini-fill {
  opacity: 1;
}

.gemini-fill.fill-complete {
  mask: none;
}

/* Phase 3: Breathing ripples */
.gemini-ripple {
  opacity: 0;
  pointer-events: none;
}

.gemini-ripple.ripple-active {
  animation: ripple-expand 4s cubic-bezier(0, 0, 0.2, 1) infinite;
}

.gemini-ripple.ripple-active.d-1 {
  animation-name: ripple-expand;
  animation-delay: 0s;
}
.gemini-ripple.ripple-active.d-2 {
  animation-name: ripple-expand-up;
  animation-delay: 1.3s;
}
.gemini-ripple.ripple-active.d-3 {
  animation-name: ripple-expand-diagonal;
  animation-delay: 2.6s;
}

/* Aether Fill Animation */
.aether-fill {
  opacity: 0;
  transition: opacity 1.5s ease;
  pointer-events: none;
}

.aether-fill.fill-active {
  opacity: 0.6; /* Solid fill to show logo shape clearly */
}

/* Aether Breathing Animation */
@keyframes aether-breathe {
  0%, 100% {
    stroke-width: 0.6px;
    opacity: 0.85;
    transform: scale(1);
  }
  50% {
    stroke-width: 1.5px;
    opacity: 1;
    transform: scale(1.05);
  }
}

@keyframes aether-glow-pulse {
  0%, 100% {
    filter: url(#aether-glow) brightness(1) drop-shadow(0 0 2px rgba(204, 120, 92, 0.3));
  }
  50% {
    filter: url(#aether-glow) brightness(1.3) drop-shadow(0 0 8px rgba(204, 120, 92, 0.6));
  }
}

.aether-stroke.breathing {
  animation: aether-breathe 4s ease-in-out infinite;
  transition: stroke 0.5s ease;
}

.aether-fill.breathing {
  animation: aether-fill-breathe 4s ease-in-out infinite;
}

@keyframes aether-fill-breathe {
  0%, 100% {
    opacity: 0.6;
    transform: scale(1);
  }
  50% {
    opacity: 0.75;
    transform: scale(1.05);
  }
}

/* Logo transition styles - simple fade only, no transform to avoid animation interference */
.logo-fade-enter-active,
.logo-fade-leave-active {
  transition: opacity 0.3s ease;
}

.logo-fade-enter-from,
.logo-fade-leave-to {
  opacity: 0;
}

/* Adaptive Aether Logo Styles - 3 Phase Animation */
/* Phase 1: Stroke drawing -> Phase 2: Fill reveal -> Phase 3: Ripple breathing */
.aether-adaptive-container {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* Stroke drawing animation keyframes */
@keyframes adaptive-stroke-draw {
  0% {
    stroke-dashoffset: -12000;
    opacity: 0.3;
  }
  10% {
    opacity: 1;
  }
  100% {
    stroke-dashoffset: 0;
    opacity: 1;
  }
}

/* Ripple layers - hidden until fill completes (Phase 3) */
.adaptive-ripple {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  pointer-events: none;
  visibility: hidden;
}

.adaptive-ripple.active {
  visibility: visible;
  animation: adaptive-ripple-expand 4s ease-out infinite;
}

.adaptive-logo-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

@keyframes adaptive-ripple-expand {
  0% {
    transform: scale(1);
    opacity: 0.35;
  }
  100% {
    transform: scale(2);
    opacity: 0;
  }
}

@keyframes adaptive-ripple-expand-up {
  0% {
    transform: scale(1) translateY(0);
    opacity: 0.35;
  }
  100% {
    transform: scale(1.8) translateY(-25%);
    opacity: 0;
  }
}

@keyframes adaptive-ripple-expand-diagonal {
  0% {
    transform: scale(1) translate(0, 0);
    opacity: 0.35;
  }
  100% {
    transform: scale(2) translate(12%, -12%);
    opacity: 0;
  }
}

/* Stagger the ripples with different directions */
.adaptive-ripple.active.r-1 {
  animation-name: adaptive-ripple-expand;
  animation-delay: 0s;
}
.adaptive-ripple.active.r-2 {
  animation-name: adaptive-ripple-expand-up;
  animation-delay: 1.33s;
}
.adaptive-ripple.active.r-3 {
  animation-name: adaptive-ripple-expand-diagonal;
  animation-delay: 2.66s;
}

/* Phase 1: Stroke overlay - positioned above fill layer */
.adaptive-stroke-overlay {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  z-index: 3;
  pointer-events: none;
  overflow: visible;
}

/* Fade out stroke overlay when fill starts */
.adaptive-stroke-overlay.stroke-complete {
  opacity: 0;
  transition: opacity 0.5s ease;
}

/* Stroke path animation */
.adaptive-stroke-path {
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-dasharray: 12000;
  stroke-dashoffset: -12000;
  animation: adaptive-stroke-draw 1.5s cubic-bezier(0.4, 0, 0.2, 1) forwards;
}

/* Phase 2: Fill layer using original SVG image */
.adaptive-fill-layer {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  z-index: 2;
  overflow: visible;
  opacity: 0;
}

.adaptive-fill-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

/* Fade in the fill - use ease-in-out for smoother transition to breathing */
.adaptive-fill-layer.fill-active {
  animation: adaptive-fill-fadein 0.6s ease-in-out forwards;
}

.adaptive-fill-layer.fill-complete {
  opacity: 1;
}

@keyframes adaptive-fill-fadein {
  0% {
    opacity: 0;
    transform: scale(0.98);
  }
  100% {
    opacity: 1;
    transform: scale(1);
  }
}

/* Static mode fill with fade-in animation */
.static-fill {
  animation: static-fill-fadein 2s ease-in forwards;
}

@keyframes static-fill-fadein {
  0% {
    opacity: 0;
  }
  100% {
    opacity: 1;
  }
}

</style>
