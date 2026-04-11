<script setup lang="ts">
import { Button } from '~/components/ui/button'
import { Badge } from '~/components/ui/badge'
import { Textarea } from '~/components/ui/textarea'
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from '~/components/ui/accordion'
import { Separator } from '~/components/ui/separator'
import type { SchemaKnowledgeBase, TableDescription } from '~/types/database'

const props = defineProps<{
  kb: SchemaKnowledgeBase
}>()

const emit = defineEmits<{
  'update-description': [tableDescId: string, description: string]
  'refresh': []
}>()

const editingId = ref<string | null>(null)
const editValue = ref('')

function startEdit(tableDesc: { id: string; ai_description: string | null }) {
  editingId.value = tableDesc.id
  editValue.value = tableDesc.ai_description ?? ''
}

function saveEdit(id: string) {
  emit('update-description', id, editValue.value)
  editingId.value = null
}

function cancelEdit() {
  editingId.value = null
}

function getColumnMetadata(columnMetadata: string) {
  try {
    return JSON.parse(columnMetadata) as Array<{
      name: string
      data_type: string
      is_nullable: boolean
      is_primary_key: boolean
      is_foreign_key: boolean
      referenced_table?: string
      referenced_column?: string
    }>
  }
  catch {
    return []
  }
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <!-- KB Header -->
    <div class="flex items-center justify-between">
      <div>
        <h3 class="font-semibold flex items-center gap-2">
          <Icon name="lucide:sparkles" class="size-4 text-primary" />
          AI Knowledge Base
        </h3>
        <p v-if="kb.snapshot.summary" class="text-sm text-muted-foreground mt-1 line-clamp-2">
          {{ kb.snapshot.summary }}
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Badge
          :variant="kb.snapshot.status === 'ready' ? 'default' : 'secondary'"
          class="text-xs"
        >
          {{ kb.snapshot.status === 'ready' ? 'Ready' : kb.snapshot.status }}
        </Badge>
        <Button size="sm" variant="ghost" @click="emit('refresh')">
          <Icon name="lucide:refresh-cw" class="size-3.5" />
        </Button>
      </div>
    </div>

    <Separator />

    <!-- Relationships summary -->
    <div v-if="kb.relationships.length > 0" class="text-sm">
      <p class="font-medium text-muted-foreground mb-2">
        {{ kb.relationships.length }} relationship{{ kb.relationships.length > 1 ? 's' : '' }} detected
      </p>
      <div class="flex flex-wrap gap-1.5">
        <Badge
          v-for="rel in kb.relationships.slice(0, 10)"
          :key="`${rel.source_table}-${rel.source_column}`"
          variant="outline"
          class="text-xs font-mono"
        >
          {{ rel.source_table }}.{{ rel.source_column }} → {{ rel.target_table }}.{{ rel.target_column }}
        </Badge>
      </div>
    </div>

    <!-- Tables accordion -->
    <Accordion type="multiple" class="space-y-1">
      <AccordionItem
        v-for="table in kb.tables"
        :key="table.id"
        :value="table.id"
        class="rounded-md border border-border bg-card"
      >
        <AccordionTrigger class="px-3 py-2 hover:no-underline">
          <div class="flex items-center gap-2 text-left">
            <Icon name="lucide:table-2" class="size-4 text-muted-foreground shrink-0" />
            <span class="font-medium text-sm">{{ table.table_name }}</span>
            <Badge v-if="table.schema_name" variant="outline" class="text-xs">{{ table.schema_name }}</Badge>
            <Badge variant="secondary" class="text-xs">{{ table.table_type }}</Badge>
          </div>
        </AccordionTrigger>
        <AccordionContent class="px-3 pb-3 space-y-3">
          <!-- AI Description -->
          <div class="space-y-1">
            <div class="flex items-center justify-between">
              <span class="text-xs font-medium text-muted-foreground uppercase tracking-wide">AI Description</span>
              <Button
                v-if="editingId !== table.id"
                size="sm"
                variant="ghost"
                class="h-6 px-2 text-xs"
                @click="startEdit(table)"
              >
                <Icon name="lucide:pencil" class="size-3" />
                Edit
              </Button>
            </div>

            <div v-if="editingId === table.id" class="space-y-2">
              <Textarea
                v-model="editValue"
                class="text-sm min-h-[80px] resize-none"
                placeholder="Describe what this table represents..."
              />
              <div class="flex gap-1.5">
                <Button size="sm" class="h-7 text-xs" @click="saveEdit(table.id)">
                  Save
                </Button>
                <Button size="sm" variant="ghost" class="h-7 text-xs" @click="cancelEdit">
                  Cancel
                </Button>
              </div>
            </div>
            <p v-else class="text-sm text-foreground leading-relaxed">
              {{ table.ai_description ?? 'No AI description generated yet.' }}
            </p>
          </div>

          <!-- Columns -->
          <div class="space-y-1">
            <span class="text-xs font-medium text-muted-foreground uppercase tracking-wide">Columns</span>
            <div class="overflow-auto max-h-48 rounded-md bg-muted/30 p-1.5">
              <table class="w-full text-xs">
                <tbody class="divide-y divide-border/50">
                  <tr
                    v-for="col in getColumnMetadata(table.column_metadata)"
                    :key="col.name"
                  >
                    <td class="py-1 pr-2 font-mono text-foreground">{{ col.name }}</td>
                    <td class="py-1 pr-2 text-muted-foreground">{{ col.data_type }}</td>
                    <td class="py-1">
                      <div class="flex gap-1">
                        <Badge v-if="col.is_primary_key" class="text-[10px] py-0 h-4">PK</Badge>
                        <Badge v-if="col.is_foreign_key" variant="secondary" class="text-[10px] py-0 h-4">
                          FK→{{ col.referenced_table }}
                        </Badge>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  </div>
</template>
