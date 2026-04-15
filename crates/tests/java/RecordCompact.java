record Range(int lo, int hi) {
    Range {
        if (lo > hi) {
            throw new IllegalArgumentException("lo must be <= hi");
        }
    }

    String describe() {
        return lo + ".." + hi;
    }
}

class RecordCompact {
    public static void main(String[] args) {
        Range r = new Range(1, 10);
        System.out.println(r.describe());
        System.out.println(r);

        // Verify validation runs: this should throw
        try {
            Range bad = new Range(10, 1);
            System.out.println("should not reach");
        } catch (IllegalArgumentException e) {
            System.out.println("caught: " + e.getMessage());
        }
    }
}
