import { useParams } from "react-router-dom";
import NotFoundError from "@/pages/errors/not-found-error";
import MessageRadarPage from "./message-radar";
import ContractReviewPage from "./contract-review";

export default function BuiltinPage() {
	const { id } = useParams<{ id: string }>();
	if (id === "message-radar") return <MessageRadarPage />;
	if (id === "contract-review") return <ContractReviewPage />;
	return <NotFoundError />;
}
