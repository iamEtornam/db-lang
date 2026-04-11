<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Label } from '~/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '~/components/ui/select'

const props = defineProps<{
  data: string
  columns: string[]
}>()

const open = defineModel<boolean>('open', { default: false })

const format = ref<'csv' | 'json' | 'xlsx'>('csv')
const isExporting = ref(false)

async function doExport() {
  isExporting.value = true

  try {
    const result = await invoke<string>('export_data', {
      data: props.data,
      format: format.value,
      options: {
        format: format.value,
        include_headers: true,
        delimiter: ',',
      },
    })

    // Create download
    const mimeTypes: Record<string, string> = {
      csv: 'text/csv',
      json: 'application/json',
      xlsx: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    }

    if (format.value === 'xlsx') {
      // Result is base64 for binary formats
      const bytes = Uint8Array.from(atob(result), c => c.charCodeAt(0))
      const blob = new Blob([bytes], { type: mimeTypes[format.value] })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `export.${format.value}`
      a.click()
      URL.revokeObjectURL(url)
    }
    else {
      const blob = new Blob([result], { type: mimeTypes[format.value] })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `export.${format.value}`
      a.click()
      URL.revokeObjectURL(url)
    }

    toast.success('Export successful')
    open.value = false
  }
  catch (err) {
    toast.error('Export failed', { description: err as string })
  }
  finally {
    isExporting.value = false
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-sm">
      <DialogHeader>
        <DialogTitle>Export Results</DialogTitle>
      </DialogHeader>

      <div class="space-y-4 py-2">
        <div class="space-y-2">
          <Label>Format</Label>
          <Select v-model="format">
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="csv">CSV (Comma Separated)</SelectItem>
              <SelectItem value="json">JSON</SelectItem>
              <SelectItem value="xlsx">Excel (XLSX)</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <p class="text-xs text-muted-foreground">
          {{ columns.length }} columns · Current page data
        </p>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="open = false">Cancel</Button>
        <Button :disabled="isExporting || !data" @click="doExport">
          <Icon v-if="isExporting" name="lucide:loader-2" class="size-4 animate-spin" />
          <Icon v-else name="lucide:download" class="size-4" />
          Export
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
