import { APP_DISPLAY_NAME } from "@/brand";

export default function Home() {
  return (
    <main className="flex flex-1 items-center justify-center">
      <h1 className="text-4xl font-semibold">{APP_DISPLAY_NAME}</h1>
    </main>
  );
}
