<template>
  <Card :class="cardClasses">
    <div
      v-if="title || description || $slots.header"
      :class="headerClasses"
    >
      <slot name="header">
        <div class="flex items-center justify-between">
          <div>
            <h3
              v-if="title"
              class="text-lg font-medium leading-6 text-foreground"
            >
              {{ title }}
            </h3>
            <p
              v-if="description"
              class="mt-1 text-sm text-muted-foreground"
            >
              {{ description }}
            </p>
          </div>
          <div v-if="$slots.actions">
            <slot name="actions" />
          </div>
        </div>
      </slot>
    </div>

    <div :class="contentClasses">
      <slot />
    </div>

    <div
      v-if="$slots.footer"
      :class="footerClasses"
    >
      <slot name="footer" />
    </div>
  </Card>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import Card from '@/components/ui/card.vue'

interface Props {
  title?: string
  description?: string
  variant?: 'default' | 'elevated' | 'glass'
  padding?: 'none' | 'sm' | 'md' | 'lg'
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'default',
  padding: 'md',
})

const cardClasses = computed(() => {
  const classes = []

  if (props.variant === 'elevated') {
    classes.push('shadow-md')
  } else if (props.variant === 'glass') {
    classes.push('surface-glass')
  }

  return classes.join(' ')
})

const headerClasses = computed(() => {
  const paddingMap = {
    none: '',
    sm: 'px-3 py-3',
    md: 'px-4 py-5 sm:p-6',
    lg: 'px-6 py-6 sm:p-8',
  }

  const classes = [paddingMap[props.padding]]

  if (props.padding !== 'none') {
    classes.push('border-b border-border')
  }

  return classes.join(' ')
})

const contentClasses = computed(() => {
  const paddingMap = {
    none: '',
    sm: 'px-3 py-3',
    md: 'px-4 py-5 sm:p-6',
    lg: 'px-6 py-6 sm:p-8',
  }

  return paddingMap[props.padding]
})

const footerClasses = computed(() => {
  const paddingMap = {
    none: '',
    sm: 'px-3 py-3',
    md: 'px-4 py-5 sm:p-6',
    lg: 'px-6 py-6 sm:p-8',
  }

  const classes = [paddingMap[props.padding]]

  if (props.padding !== 'none') {
    classes.push('border-t border-border')
  }

  return classes.join(' ')
})
</script>
