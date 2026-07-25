import { SidebarProductOpenButton } from '../../../components/product-sidebar';

export function WorkSidebarOpenButton({ onOpen }: { onOpen: () => void }) {
  return <SidebarProductOpenButton title='办公' className='work-sidebar-open-button' onOpen={onOpen} />;
}
