import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { Provider as TooltipProvider } from '@radix-ui/react-tooltip'
import './studio.css'
import Studio from './Studio.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <TooltipProvider delayDuration={350} skipDelayDuration={150}>
      <Studio />
    </TooltipProvider>
  </StrictMode>,
)
