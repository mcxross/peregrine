import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

function App() {
  return (
    <main className="flex min-h-svh items-center justify-center bg-background p-6 text-foreground">
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle>Peregrine</CardTitle>
          <CardDescription>Move Security</CardDescription>
        </CardHeader>
        <CardContent>
          <Button className="w-full">Continue</Button>
        </CardContent>
      </Card>
    </main>
  );
}

export default App;
