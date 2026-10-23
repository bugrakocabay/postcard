// Sidebar drag-and-drop state.
import { ws } from './workspace.svelte'
import { moveItem, findItem } from '../util/tree'
import { requestIdsIn, moveRequestIdsTo } from '../actions.svelte'

export type DropPos = 'before' | 'after' | 'inside'

class DndState {
  drag = $state<{ collectionId: string; itemId: string } | null>(null)
  over = $state<{ id: string; pos: DropPos } | null>(null)

  start(collectionId: string, itemId: string, e: DragEvent) {
    this.drag = { collectionId, itemId }
    e.dataTransfer?.setData('text/plain', itemId)
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
  }

  end() {
    this.drag = null
    this.over = null
  }

  /** Drop the dragged item relative to a target container/item. */
  drop(toCollectionId: string, parentId: string | null, index: number) {
    const d = this.drag
    this.end()
    if (!d) return
    const from = ws.collection(d.collectionId)
    const to = ws.collection(toCollectionId)
    if (!from || !to) return
    const item = findItem(from, d.itemId)
    if (!item) return
    if (moveItem(from, d.itemId, to, parentId, index)) {
      // Destination first: if writing stops halfway, the item exists twice rather than nowhere.
      if (to !== from) ws.saveCollection(to)
      ws.saveCollection(from)
      if (to !== from) {
        moveRequestIdsTo(requestIdsIn(item), to.id)
      }
    }
  }
}

export const dnd = new DndState()
