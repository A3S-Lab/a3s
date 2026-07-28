import { SidebarProductOpenButton } from '../../../components/product-sidebar';

export function ConversationSidebarOpenButton({ onOpen }: { onOpen: () => void }) {
  return <SidebarProductOpenButton title='会话' className='product-sidebar-open-button' onOpen={onOpen} />;
}
