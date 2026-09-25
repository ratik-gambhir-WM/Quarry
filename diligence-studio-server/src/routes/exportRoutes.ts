import { raw, Router } from 'express'

import { createExportHandlers } from '../handlers/exportHandlers'
import type { ExportPowerPointUseCase } from '../services/ExportPowerPointService'

export function createExportRouter(
  service: ExportPowerPointUseCase,
  maxJsonBytes: number,
  maxTargetBytes: number,
) {
  const router = Router()
  const handlers = createExportHandlers(service)

  router.post('/insert', handlers.insert(maxJsonBytes, maxTargetBytes))
  router.all('/insert', (_request, response) => {
    response.status(405).end()
  })
  router.post(
    '/',
    raw({ inflate: false, limit: maxJsonBytes, type: 'application/json' }),
    handlers.create,
  )
  router.all('/', (_request, response) => {
    response.status(405).end()
  })

  return router
}
