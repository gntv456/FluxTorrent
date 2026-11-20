/** 工单视图状态组（从 staff-box.tsx 拆出，300 行门禁）：
 *  状态筛选/行数据/编辑态/表单/updateTicket 动作。 */
import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useTickets } from "@/components/staff-box-desk";

export function useStaffTickets(updatedLabel: string, netErr: string) {
  const [ticketStatus, setTicketStatus] = useState<string>(""); // ""=全部
  const { ticketRows, ticketErr, loadTickets } = useTickets(ticketStatus);
  const [ticketMsg, setTicketMsg] = useState<string | null>(null);
  const [ticketBusy, setTicketBusy] = useState(false);
  const [ticketEdit, setTicketEdit] = useState<number | null>(null);
  const [ticketForm, setTicketForm] = useState({
    priority: "",
    ticket_status: "",
    assign: "",
  });

  async function updateTicket(id: number) {
    setTicketBusy(true);
    setTicketMsg(null);
    try {
      await api.post("/api/v1/stafftickets/update", {
        id,
        priority: ticketForm.priority !== ""
          ? Number(ticketForm.priority)
          : undefined,
        ticket_status:
          ticketForm.ticket_status !== ""
            ? Number(ticketForm.ticket_status)
            : undefined,
        assign: ticketForm.assign,
      });
      setTicketMsg(updatedLabel);
      setTicketEdit(null);
      setTicketForm({ priority: "", ticket_status: "", assign: "" });
      loadTickets();
    } catch (e) {
      setTicketMsg(e instanceof ApiError ? e.message : netErr);
    } finally {
      setTicketBusy(false);
    }
  }

  return {
    ticketStatus, setTicketStatus,
    ticketRows, ticketErr, loadTickets,
    ticketMsg, ticketBusy, ticketEdit, setTicketEdit, ticketForm, setTicketForm,
    updateTicket,
  };
}
