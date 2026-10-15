import type { Element, TableDimensions, Theme } from './types'
import { cssColor, fontFamily, runStyle } from './design'
import { RichTextSurface } from './RichTextSurface'

function dimension(dimensions: TableDimensions | null | undefined, index: number, count: number) {
  if (!dimensions) return `${100 / count}%`
  if (dimensions.unit === 'absolute') return `${dimensions.values[index]}px`
  const sum = dimensions.values.reduce((total, value) => total + value, 0)
  return `${sum > 0 ? dimensions.values[index] / sum * 100 : 100 / count}%`
}

export function TableSurface({ element, theme }: { element: Extract<Element, { type: 'table' }>; theme?: Theme }) {
  const format = element.format
  const columns = element.rows[0]?.length ?? 0
  const cells = new Map(format?.cells?.map((cell) => [`${cell.row}:${cell.column}`, cell.style]))
  const anchors = new Map(format?.merges?.map((merge) => [`${merge.row}:${merge.column}`, merge]))
  const followers = new Set<string>()
  for (const merge of format?.merges ?? []) {
    for (let row = merge.row; row < merge.row + merge.row_span; row++) {
      for (let column = merge.column; column < merge.column + merge.col_span; column++) {
        if (row !== merge.row || column !== merge.column) followers.add(`${row}:${column}`)
      }
    }
  }
  return <table className="slide-table document-table" style={{ fontSize: element.font_size, fontFamily: fontFamily(null, theme) }}>
    <colgroup>{Array.from({ length: columns }, (_, index) => <col key={index} style={{ width: dimension(format?.column_widths, index, columns) }} />)}</colgroup>
    <tbody>{element.rows.map((row, rowIndex) => <tr key={rowIndex} style={{ height: dimension(format?.row_heights, rowIndex, element.rows.length) }}>{row.map((text, column) => {
      const key = `${rowIndex}:${column}`
      if (followers.has(key)) return null
      const style = cells.get(key)
      const merge = anchors.get(key)
      const Cell = rowIndex === 0 ? 'th' : 'td'
      return <Cell key={column} rowSpan={merge?.row_span} colSpan={merge?.col_span} style={{
        background: cssColor(style?.fill ?? (rowIndex === 0 ? '@accent1' : rowIndex % 2 === 0 ? '@lt2' : '@lt1'), theme),
        color: cssColor(rowIndex === 0 ? '@lt1' : '@dk1', theme),
        fontWeight: rowIndex === 0 ? 700 : 400,
        border: style?.outline ? `${style.outline.width}px solid ${cssColor(style.outline.color, theme)}` : undefined,
        padding: style?.padding ? `${style.padding.top}px ${style.padding.right}px ${style.padding.bottom}px ${style.padding.left}px` : undefined,
        verticalAlign: style?.vertical ?? style?.text_format?.vertical ?? 'top',
        fontStyle: style?.text_format?.italic ? 'italic' : undefined,
        fontFamily: style?.text_format?.font_family ? fontFamily(style.text_format.font_family, theme) : undefined,
        ...runStyle({ ...style?.text_style, highlight: null, baseline: null, underline: null }, theme, element.font_size),
      }}><RichTextSurface text={text} format={style?.text_format} size={style?.text_style?.font_size ?? element.font_size} theme={theme} baseStyle={style?.text_style} /></Cell>
    })}</tr>)}</tbody>
  </table>
}