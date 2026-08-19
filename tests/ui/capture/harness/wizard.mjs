// Spawn Wizard capture helpers (plan §5 Phase 4b item 2).
// openGroup() opens a registry-generated tier group by its stable
// [data-mlx-wiz-group] id, regardless of which tier the group defaults to
// open/closed at — so scenarios don't fork on the current profile just to
// reach a control inside an Advanced-tier group.

export async function openGroup(page, groupId) {
    return page.evaluate((id) => {
        const el = document.querySelector(`[data-mlx-wiz-group="${id}"]`);
        if (el && el.tagName === 'DETAILS' && !el.open) {
            el.open = true;
        }
        return !!el;
    }, groupId);
}
